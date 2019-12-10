use sparkle::gl;
use sparkle::gl::Gl;

use glib::glib_bool_error;
use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::object::Cast;
use glib::object::ObjectType;
use glib::subclass::object::ObjectImpl;
use glib::subclass::object::ObjectImplExt;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
use glib::translate::FromGlibPtrBorrow;
use gstreamer::gst_debug;
use gstreamer::gst_element_error;
use gstreamer::gst_info;
use gstreamer::gst_loggable_error;
use gstreamer::subclass::element::ElementClassSubclassExt;
use gstreamer::subclass::element::ElementImpl;
use gstreamer::subclass::ElementInstanceStruct;
use gstreamer::Buffer;
use gstreamer::BufferPool;
use gstreamer::BufferPoolExt;
use gstreamer::BufferPoolExtManual;
use gstreamer::Caps;
use gstreamer::CoreError;
use gstreamer::DebugCategory;
use gstreamer::DebugColorFlags;
use gstreamer::Element;
use gstreamer::FlowError;
use gstreamer::Format;
use gstreamer::Fraction;
use gstreamer::LoggableError;
use gstreamer::PadDirection;
use gstreamer::PadPresence;
use gstreamer::PadTemplate;
use gstreamer_base::subclass::base_src::BaseSrcImpl;
use gstreamer_base::BaseSrc;
use gstreamer_base::BaseSrcExt;
use gstreamer_gl::GLContext;
use gstreamer_gl::GLContextExt;
use gstreamer_gl::GLContextExtManual;
use gstreamer_gl_sys::gst_is_gl_memory;
use gstreamer_gl_sys::GstGLMemory;
use gstreamer_video::VideoInfo;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::Instant;

const CAPS: &str = "video/x-raw(memory:GLMemory),
  format={RGBA,RGBx},
  width=[1,2147483647],
  height=[1,2147483647],
  framerate=[0/1,2147483647/1]";

pub struct MyGLSrc {
    cat: DebugCategory,
    start: Instant,
    frame_micros: AtomicU64,
    next_frame_micros: AtomicU64,
    frames: AtomicU64,
    buffer_pool: Mutex<Option<BufferPool>>,
    out_info: Mutex<Option<VideoInfo>>,
}

impl ObjectSubclass for MyGLSrc {
    const NAME: &'static str = "MyGLSrc";
    type ParentType = BaseSrc;
    type Instance = ElementInstanceStruct<Self>;
    type Class = ClassStruct<Self>;

    fn new() -> Self {
        Self {
            cat: DebugCategory::new("myglsrc", DebugColorFlags::empty(), Some("My glsrc by me")),
            start: Instant::now(),
            frame_micros: AtomicU64::new(16_667), // Default 60fps
            next_frame_micros: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            buffer_pool: Mutex::new(None),
            out_info: Mutex::new(None),
        }
    }

    fn class_init(klass: &mut ClassStruct<Self>) {
        klass.set_metadata(
            "My GLSrc By Me",
            "Filter/Effect/Converter/Video",
            "Does stuff",
            env!("CARGO_PKG_AUTHORS"),
        );

        let src_caps = Caps::from_string(CAPS).unwrap();
        let src_pad_template =
            PadTemplate::new("src", PadDirection::Src, PadPresence::Always, &src_caps).unwrap();
        klass.add_pad_template(src_pad_template);
    }

    glib_object_subclass!();
}

impl ObjectImpl for MyGLSrc {
    glib_object_impl!();

    fn constructed(&self, obj: &glib::Object) {
        self.parent_constructed(obj);
        let basesrc = obj.downcast_ref::<BaseSrc>().unwrap();
        basesrc.set_live(true);
        basesrc.set_format(Format::Time);
        basesrc.set_do_timestamp(true);
    }
}

impl ElementImpl for MyGLSrc {}

thread_local! {
    static GL: RefCell<Option<Rc<Gl>>> = RefCell::new(None);
}

impl BaseSrcImpl for MyGLSrc {
    fn set_caps(&self, src: &BaseSrc, outcaps: &Caps) -> Result<(), LoggableError> {
        let out_info = VideoInfo::from_caps(outcaps)
            .ok_or_else(|| gst_loggable_error!(self.cat, "Failed to get video info"))?;
        gst_debug!(self.cat, obj: src, "Configured for caps {}", outcaps);
        *self.out_info.lock().unwrap() = Some(out_info);

        // Get the downstream GL context
        let mut gst_gl_context = std::ptr::null_mut();
        let el = src.upcast_ref::<Element>();
        unsafe {
            gstreamer_gl_sys::gst_gl_query_local_gl_context(
                el.as_ptr(),
                gstreamer_sys::GST_PAD_SRC,
                &mut gst_gl_context,
            );
        }
        if gst_gl_context.is_null() {
            return Err(gst_loggable_error!(self.cat, "Failed to get GL context"));
        }

        // Create a new buffer pool for GL memory
        let gst_gl_buffer_pool =
            unsafe { gstreamer_gl_sys::gst_gl_buffer_pool_new(gst_gl_context) };
        if gst_gl_buffer_pool.is_null() {
            return Err(gst_loggable_error!(
                self.cat,
                "Failed to create buffer pool"
            ));
        }
        let pool = unsafe { BufferPool::from_glib_borrow(gst_gl_buffer_pool) };

        // Configure the buffer pool with the negotiated caps
        let mut config = pool.get_config();
        let (_, size, min_buffers, max_buffers) = config.get_params().unwrap_or((None, 0, 0, 1024));
        config.set_params(Some(outcaps), size, min_buffers, max_buffers);
        pool.set_config(config)
            .map_err(|_| gst_loggable_error!(self.cat, "Failed to update config"))?;

        // Save the buffer pool for later use
        *self.buffer_pool.lock().expect("Poisoned lock") = Some(pool);

        // Is the framerate set?
        let framerate = outcaps
            .get_structure(0)
            .and_then(|cap| cap.get::<Fraction>("framerate"));
        if let Some(framerate) = framerate {
            let frame_micros = 1_000_000 * *framerate.denom() as u64 / *framerate.numer() as u64;
            gst_debug!(
                self.cat,
                obj: src,
                "Setting frame duration to {}micros",
                frame_micros
            );
            self.frame_micros.store(frame_micros, Ordering::SeqCst);
        }
        Ok(())
    }

    fn is_seekable(&self, _: &BaseSrc) -> bool {
        false
    }

    fn create(&self, src: &BaseSrc, _offset: u64, _length: u32) -> Result<Buffer, FlowError> {
        // We block waiting for the next frame to be needed.
        // Once get_times is in BaseSrcImpl, we can use that instead.
        // It's been merged but not yet published.
        // https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/merge_requests/375
        let elapsed_micros = self.start.elapsed().as_micros() as u64;
        let frame_micros = self.frame_micros.load(Ordering::SeqCst);
        let next_frame_micros = self
            .next_frame_micros
            .fetch_add(frame_micros, Ordering::SeqCst);
        if elapsed_micros < next_frame_micros {
            let delay = 1_000_000.min(next_frame_micros - elapsed_micros);
            gst_debug!(self.cat, obj: src, "Waiting for {}micros", delay);
            thread::sleep(Duration::from_micros(delay));
            gst_debug!(self.cat, obj: src, "Done waiting");
        }

        // Get the buffer pool
        let pool_guard = self.buffer_pool.lock().unwrap();
        let pool = pool_guard.as_ref().ok_or(FlowError::NotNegotiated)?;

        // Activate the pool if necessary
        if !pool.is_active() {
            pool.set_active(true).map_err(|_| FlowError::Error)?;
        }

        // Get a buffer to fill
        let buffer = pool.acquire_buffer(None)?;

        // Get the GL memory from the buffer
        let memory = buffer.get_all_memory().ok_or_else(|| {
            gst_element_error!(src, CoreError::Failed, ["Failed to get memory"]);
            FlowError::Error
        })?;
        let memory = unsafe { memory.into_ptr() };
        if unsafe { gst_is_gl_memory(memory) } == 0 {
            gst_element_error!(src, CoreError::Failed, ["Memory isn't GL memory"]);
            return Err(FlowError::Error);
        }
        let gl_memory = unsafe { (memory as *mut GstGLMemory).as_ref() }.ok_or_else(|| {
            gst_element_error!(src, CoreError::Failed, ["Memory is null"]);
            FlowError::Error
        })?;

        // Get the data out of the memory
        let gl_context = unsafe { GLContext::from_glib_borrow(gl_memory.mem.context) };
        let draw_texture_id = gl_memory.tex_id;
        let height = gl_memory.info.height;
        let width = gl_memory.info.width;

        // Get the GL bindings
        let gl = GL.with(|gl| {
            gl.borrow_mut()
                .get_or_insert_with(|| {
                    Gl::gl_fns(gl::ffi_gl::Gl::load_with(|s| {
                        gl_context.get_proc_address(s) as *const _
                    }))
                })
                .clone()
        });
        gl_context.activate(true).unwrap();
        assert_eq!(gl.get_error(), gl::NO_ERROR);

        gst_debug!(
            self.cat,
            obj: src,
            "Filling mysrc buffer {}x{} {}",
            width,
            height,
            draw_texture_id,
        );

        let millis = self.start.elapsed().subsec_millis();
        let brightness = if millis < 500 {
            (millis as f32) / 500.0
        } else {
            (1000.0 - millis as f32) / 500.0
        };

        let draw_fbo = gl.gen_framebuffers(1)[0];
        assert_eq!(gl.get_error(), gl::NO_ERROR);

        gl.bind_framebuffer(gl::FRAMEBUFFER, draw_fbo);
        gl.framebuffer_texture_2d(
            gl::FRAMEBUFFER,
            gl::COLOR_ATTACHMENT0,
            gl::TEXTURE_2D,
            draw_texture_id,
            0,
        );
        assert_eq!(gl.get_error(), gl::NO_ERROR);

        gl.clear_color(brightness, brightness, brightness, 1.0);
        gl.clear(gl::COLOR_BUFFER_BIT);
        assert_eq!(gl.get_error(), gl::NO_ERROR);

        gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
        gl.delete_framebuffers(&[draw_fbo]);
        assert_eq!(gl.get_error(), gl::NO_ERROR);

        let frames = self.frames.fetch_add(1, Ordering::SeqCst);
        if frames % 100 == 0 {
            let fps = (frames * 1_000_000) / elapsed_micros;
            gst_info!(self.cat, obj: src, "fps = {}", fps);
        }

        Ok(buffer)
    }
}
