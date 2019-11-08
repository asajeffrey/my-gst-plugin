use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;

use euclid::default::Size2D;

use glib::glib_bool_error;
use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::subclass::object::ObjectImpl;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
use gstreamer::gst_debug;
use gstreamer::gst_element_error;
use gstreamer::gst_error_msg;
use gstreamer::gst_loggable_error;
use gstreamer::subclass::element::ElementClassSubclassExt;
use gstreamer::subclass::element::ElementImpl;
use gstreamer::subclass::ElementInstanceStruct;
use gstreamer::BufferRef;
use gstreamer::Caps;
use gstreamer::CoreError;
use gstreamer::DebugCategory;
use gstreamer::DebugColorFlags;
use gstreamer::ErrorMessage;
use gstreamer::FlowError;
use gstreamer::FlowSuccess;
use gstreamer::Fraction;
use gstreamer::FractionRange;
use gstreamer::IntRange;
use gstreamer::LoggableError;
use gstreamer::PadDirection;
use gstreamer::PadPresence;
use gstreamer::PadTemplate;
use gstreamer_base::subclass::base_src::BaseSrcImpl;
use gstreamer_base::BaseSrc;
use gstreamer_video::VideoFormat;
use gstreamer_video::VideoFrameRef;
use gstreamer_video::VideoInfo;

use sparkle::gl;
use sparkle::gl::Gl;

use surfman::platform::generic::universal::context::Context;
use surfman::platform::generic::universal::device::Device;
use surfman::SurfaceAccess;

use surfman_chains::SwapChain;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

pub struct MyGLSrc {
    cat: DebugCategory,
    sender: Sender<MyGLMsg>,
    swap_chain: SwapChain,
    info: Mutex<Option<VideoInfo>>,
}

struct MyGfx {
    device: Device,
    context: Context,
    gl: Rc<Gl>,
}

impl MyGfx {
    fn new() -> MyGfx {
        let connection = surfman::Connection::new().expect("Failed to create connection");
        let adapter = surfman::Adapter::default().expect("Failed to create adapter");
        let mut device =
            surfman::Device::new(&connection, &adapter).expect("Failed to create device");
        let descriptor = device
            .create_context_descriptor(&ATTRIBUTES)
            .expect("Failed to create descriptor");
        let context = device
            .create_context(&descriptor)
            .expect("Failed to create context");
        let device = Device::Hardware(device);
        let context = Context::Hardware(context);
        let gl = Gl::gl_fns(gl::ffi_gl::Gl::load_with(|s| {
            device.get_proc_address(&context, s)
        }));
        Self {
            device,
            context,
            gl,
        }
    }
}

const GL_VERSION: surfman::GLVersion = surfman::GLVersion { major: 4, minor: 0 };
const GL_FLAGS: surfman::ContextAttributeFlags = surfman::ContextAttributeFlags::empty();
const ATTRIBUTES: surfman::ContextAttributes = surfman::ContextAttributes {
    version: GL_VERSION,
    flags: GL_FLAGS,
};

thread_local! {
    static GFX: RefCell<MyGfx> = RefCell::new(MyGfx::new());
}

enum MyGLMsg {
    GetSwapChain(Sender<SwapChain>),
    Resize(Size2D<i32>),
    Heartbeat,
}

struct MyGLThread {
    cat: DebugCategory,
    receiver: Receiver<MyGLMsg>,
    swap_chain: SwapChain,
}

impl MyGLThread {
    fn new(cat: DebugCategory, receiver: Receiver<MyGLMsg>) -> Self {
        GFX.with(|gfx| {
            let mut gfx = gfx.borrow_mut();
            let gfx = &mut *gfx;
            let access = SurfaceAccess::GPUCPU;
            let size = Size2D::new(500, 500);
            let swap_chain =
                SwapChain::create_detached(&mut gfx.device, &mut gfx.context, access, size)
                    .expect("Failed to create swap chain");
            Self {
                cat,
                receiver,
                swap_chain,
            }
        })
    }

    fn run(&mut self) {
        GFX.with(|gfx| {
            let mut gfx = gfx.borrow_mut();
            let gfx = &mut *gfx;
            while let Ok(msg) = self.receiver.recv() {
                self.handle_msg(&mut *gfx, msg);
            }
        })
    }

    fn handle_msg(&mut self, gfx: &mut MyGfx, msg: MyGLMsg) {
        match msg {
            MyGLMsg::GetSwapChain(sender) => {
                let _ = sender.send(self.swap_chain.clone());
            }
            MyGLMsg::Resize(size) => {
                let _ = self
                    .swap_chain
                    .resize(&mut gfx.device, &mut gfx.context, size);
            }
            MyGLMsg::Heartbeat => {
                gfx.device.make_context_current(&gfx.context).unwrap();
                gfx.gl.clear_color(0.3, 0.3, 1.0, 0.0);
                gfx.gl.clear(gl::COLOR_BUFFER_BIT);
                gfx.device.make_no_context_current().unwrap();
                self.swap_chain
                    .swap_buffers(&mut gfx.device, &mut gfx.context);
            }
        }
    }
}

impl Drop for MyGLThread {
    fn drop(&mut self) {
        GFX.with(|gfx| {
            let mut gfx = gfx.borrow_mut();
            let gfx = &mut *gfx;
            self.swap_chain
                .destroy(&mut gfx.device, &mut gfx.context)
                .expect("Failed to destroy swap chain")
        })
    }
}

impl ObjectSubclass for MyGLSrc {
    const NAME: &'static str = "MyGLSrc";
    // gstreamer-gl doesn't have support for GLBaseSrc yet
    // https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/issues/219
    type ParentType = BaseSrc;
    type Instance = ElementInstanceStruct<Self>;
    type Class = ClassStruct<Self>;

    fn new() -> Self {
        let cat = DebugCategory::new("myglsrc", DebugColorFlags::empty(), Some("My glsrc by me"));
        let (sender, receiver) = crossbeam_channel::unbounded();
        thread::spawn(move || MyGLThread::new(cat, receiver).run());
        let (acks, ackr) = crossbeam_channel::bounded(1);
        let _ = sender.send(MyGLMsg::GetSwapChain(acks));
        let swap_chain = ackr.recv().expect("Failed to get swap chain");
        let info = Mutex::new(None);
        Self {
            cat,
            sender,
            swap_chain,
            info,
        }
    }

    fn class_init(klass: &mut ClassStruct<Self>) {
        klass.set_metadata(
            "My GLSrc By Me",
            "Filter/Effect/Converter/Video",
            "Does stuff",
            env!("CARGO_PKG_AUTHORS"),
        );

        let src_caps = Caps::new_simple(
            "video/x-raw",
            &[
                ("format", &VideoFormat::Bgrx.to_string()),
                ("width", &IntRange::<i32>::new(512, 1024)),
                ("height", &IntRange::<i32>::new(512, 1024)),
                (
                    "framerate",
                    &FractionRange::new(Fraction::new(25, 1), Fraction::new(120, 1)),
                ),
            ],
        );
        let src_pad_template =
            PadTemplate::new("src", PadDirection::Src, PadPresence::Always, &src_caps).unwrap();
        klass.add_pad_template(src_pad_template);
    }

    glib_object_subclass!();
}

impl ObjectImpl for MyGLSrc {
    glib_object_impl!();
}

impl ElementImpl for MyGLSrc {}

impl BaseSrcImpl for MyGLSrc {
    fn set_caps(&self, src: &BaseSrc, outcaps: &Caps) -> Result<(), LoggableError> {
        let info = VideoInfo::from_caps(outcaps)
            .ok_or_else(|| gst_loggable_error!(self.cat, "Failed to get video info"))?;
        let size = Size2D::new(info.height(), info.width()).to_i32();
        self.sender
            .send(MyGLMsg::Resize(size))
            .map_err(|_| gst_loggable_error!(self.cat, "Failed to send video info"))?;
        *self.info.lock().unwrap() = Some(info);
        Ok(())
    }

    fn fill(
        &self,
        src: &BaseSrc,
        _offset: u64,
        _length: u32,
        buffer: &mut BufferRef,
    ) -> Result<FlowSuccess, FlowError> {
        let guard = self.info.lock().map_err(|_| {
            gst_element_error!(src, CoreError::Negotiation, ["Lock poisoned"]);
            FlowError::NotNegotiated
        })?;
        let info = guard.as_ref().ok_or_else(|| {
            gst_element_error!(src, CoreError::Negotiation, ["Caps not set yet"]);
            FlowError::NotNegotiated
        })?;
        gst_debug!(self.cat, obj: src, "Filling myglsrc buffer {:?}", buffer,);
        let mut frame = VideoFrameRef::from_buffer_ref_writable(buffer, info).ok_or_else(|| {
            gst_element_error!(
                src,
                CoreError::Failed,
                ["Failed to map output buffer writable"]
            );
            FlowError::Error
        })?;
        let height = frame.height() as i32;
        let width = frame.width() as i32;
        let stride = frame.plane_stride()[0] as usize;
        let format = frame.format();
        gst_debug!(
            self.cat,
            obj: src,
            "Filling myglsrc buffer {}x{} {:?} {:?}",
            width,
            height,
            format,
            frame,
        );
        let data = frame.plane_data_mut(0).unwrap();

        GFX.with(|gfx| {
            let mut gfx = gfx.borrow_mut();
            let gfx = &mut *gfx;
            if let Some(surface) = self.swap_chain.take_surface() {
                gfx.device.make_context_current(&gfx.context).unwrap();
                gfx.gl.viewport(0, 0, width, height);

                let surface_info = gfx.device.surface_info(&surface);
                let surface_texture = gfx
                    .device
                    .create_surface_texture(&mut gfx.context, surface)
                    .unwrap();
                let texture_id = surface_texture.gl_texture();

                gfx.gl.framebuffer_texture_2d(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    gfx.device.surface_gl_texture_target(),
                    texture_id,
                    0,
                );
                gfx.gl.read_pixels_into_buffer(
                    0,
                    0,
                    width,
                    height,
                    gl::BGRA,
                    gl::UNSIGNED_BYTE,
                    data,
                );

                debug_assert_eq!(gfx.gl.get_error(), gl::NO_ERROR);
                gst_debug!(self.cat, obj: src, "Read pixels {:?}", &data[..127]);

                gfx.device.make_no_context_current().unwrap();

                let surface = gfx
                    .device
                    .destroy_surface_texture(&mut gfx.context, surface_texture)
                    .unwrap();
                self.swap_chain.recycle_surface(surface);
            }
        });
        let _ = self.sender.send(MyGLMsg::Heartbeat);
        Ok(FlowSuccess::Ok)
    }
}
