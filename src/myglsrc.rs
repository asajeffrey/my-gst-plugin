use sparkle::gl;
use sparkle::gl::Gl;

use glib::glib_bool_error;
use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::object::Cast;
use glib::subclass::object::ObjectImpl;
use glib::subclass::object::ObjectImplExt;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
use glib::translate::FromGlibPtrBorrow;
use gstreamer::gst_debug;
use gstreamer::gst_element_error;
use gstreamer::gst_loggable_error;
use gstreamer::subclass::element::ElementClassSubclassExt;
use gstreamer::subclass::element::ElementImpl;
use gstreamer::subclass::ElementInstanceStruct;
use gstreamer::BufferRef;
use gstreamer::Caps;
use gstreamer::CoreError;
use gstreamer::DebugCategory;
use gstreamer::DebugColorFlags;
use gstreamer::FlowError;
use gstreamer::FlowSuccess;
use gstreamer::Format;
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
use gstreamer_gl_sys::GstGLMemory;
use gstreamer_video::VideoInfo;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Instant;

const CAPS: &str = "video/x-raw(memory:GLMemory),
  format={RGBA,RGBx},
  width=[1,2147483647],
  height=[1,2147483647],
  framerate=[0/1,2147483647/1]";

pub struct MyGLSrc {
    cat: DebugCategory,
    start: Instant,
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
        Ok(())
    }

    fn fill(
        &self,
        src: &BaseSrc,
        _offset: u64,
        _length: u32,
        buffer: &mut BufferRef,
    ) -> Result<FlowSuccess, FlowError> {
        let memory = buffer.get_all_memory().ok_or_else(|| {
            gst_element_error!(src, CoreError::Failed, ["Failed to get memory"]);
            FlowError::Error
        })?;
        let gl_memory =
            unsafe { (memory.into_ptr() as *mut GstGLMemory).as_ref() }.ok_or_else(|| {
                gst_element_error!(src, CoreError::Failed, ["Memory is null"]);
                FlowError::Error
            })?;

        let gl_context = unsafe { GLContext::from_glib_borrow(gl_memory.mem.context) };
        let draw_texture_id = gl_memory.tex_id;
        let height = gl_memory.info.height;
        let width = gl_memory.info.width;

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

        Ok(FlowSuccess::Ok)
    }
}
