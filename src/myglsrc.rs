use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;

use euclid::Size2D;

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

use surfman::platform::generic::universal::context::Context;
use surfman::platform::generic::universal::device::Device;
use surfman::SurfaceAccess;

use surfman_chains::SwapChain;

use std::cell::RefCell;
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

pub struct MyGLSrc {
    cat: DebugCategory,
    sender: Sender<MyGLMsg>,
    swap_chain: SwapChain,
}

const GL_VERSION: surfman::GLVersion = surfman::GLVersion { major: 4, minor: 0 };
const GL_FLAGS: surfman::ContextAttributeFlags = surfman::ContextAttributeFlags::empty();
const ATTRIBUTES: surfman::ContextAttributes = surfman::ContextAttributes {
    version: GL_VERSION,
    flags: GL_FLAGS,
};

thread_local! {
    static SURFMAN: RefCell<(Device, Context)> = {
        let connection = surfman::Connection::new().expect("Failed to create connection");
        let adapter  = surfman::Adapter::default().expect("Failed to create adapter");
    let mut device = surfman::Device::new(&connection, &adapter).expect("Failed to create device");
        let descriptor = device.create_context_descriptor(&ATTRIBUTES).expect("Failed to create descriptor");
    let context = device.create_context(&descriptor).expect("Failed to create context");
    RefCell::new((Device::Hardware(device), Context::Hardware(context)))
    };
}

enum MyGLMsg {
    GetSwapChain(Sender<SwapChain>),
    SetVideoInfo(VideoInfo),
}

struct MyGLThread {
    cat: DebugCategory,
    receiver: Receiver<MyGLMsg>,
    swap_chain: SwapChain,
    info: Option<VideoInfo>,
}

impl MyGLThread {
    fn new(cat: DebugCategory, receiver: Receiver<MyGLMsg>) -> Self {
        SURFMAN.with(|surfman| {
            let (ref mut device, ref mut context) = &mut *surfman.borrow_mut();
            let access = SurfaceAccess::GPUCPU;
            let size = Size2D::new(500, 500);
            let swap_chain = SwapChain::create_detached(device, context, access, size)
                .expect("Failed to create swap chain");
            let info = None;
            Self {
                cat,
                receiver,
                swap_chain,
                info,
            }
        })
    }

    fn run(&mut self) {
        while let Ok(msg) = self.receiver.recv() {
            self.handle_msg(msg);
        }
        SURFMAN.with(|surfman| {
            let (ref mut device, ref mut context) = &mut *surfman.borrow_mut();
            self.swap_chain
                .destroy(device, context)
                .expect("Failed to destroy swap chain")
        })
    }

    fn handle_msg(&mut self, msg: MyGLMsg) {
        match msg {
            MyGLMsg::GetSwapChain(sender) => {
                let _ = sender.send(self.swap_chain.clone());
            }
            MyGLMsg::SetVideoInfo(info) => {
                self.info = Some(info);
            }
        }
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
        Self {
            cat,
            sender,
            swap_chain,
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
        self.sender
            .send(MyGLMsg::SetVideoInfo(info))
            .map_err(|_| gst_loggable_error!(self.cat, "Failed to send video info"))?;
        Ok(())
    }

    fn fill(
        &self,
        src: &BaseSrc,
        _offset: u64,
        _length: u32,
        buffer: &mut BufferRef,
    ) -> Result<FlowSuccess, FlowError> {
        /*
                let out_guard = self.out_info.lock().map_err(|_| {
                    gst_element_error!(src, CoreError::Negotiation, ["Lock poisoned"]);
                    FlowError::NotNegotiated
                })?;
                let out_info = out_guard.as_ref().ok_or_else(|| {
                    gst_element_error!(src, CoreError::Negotiation, ["Caps not set yet"]);
                    FlowError::NotNegotiated
                })?;
                gst_debug!(
                    self.cat,
                    obj: src,
                    "Filling myglsrc buffer {:?}",
                    buffer,
                );
                let mut out_frame =
                    VideoFrameRef::from_buffer_ref_writable(buffer, out_info).ok_or_else(|| {
                        gst_element_error!(
                            src,
                            CoreError::Failed,
                            ["Failed to map output buffer writable"]
                        );
                        FlowError::Error
                    })?;
                let height = out_frame.height() as usize;
                let width = out_frame.width() as usize;
                let stride = out_frame.plane_stride()[0] as usize;
                let format = out_frame.format();
                gst_debug!(
                    self.cat,
                    obj: src,
                    "Filling myglsrc buffer {}x{} {:?} {:?}",
                    width,
                    height,
                    format,
                    out_frame,
                );
                let data = out_frame.plane_data_mut(0).unwrap();

                let millis = self.start.elapsed().subsec_millis();
                let brightness = if millis < 500 {
                    millis / 2
                } else {
                    (1000 - millis) / 2
                } as u8;

                if format == VideoFormat::Bgrx {
                    assert_eq!(data.len() % 4, 0);
                    let line_bytes = width * 4;
                    assert!(line_bytes <= stride);

                    for line in data.chunks_exact_mut(stride) {
                        for pixel in line[..line_bytes].chunks_exact_mut(4) {
                            pixel[0] = brightness;
                            pixel[1] = brightness / 2;
                            pixel[2] = brightness / 4;
                            pixel[3] = 0;
                        }
                    }
                } else {
                    unimplemented!();
                }
        */
        Ok(FlowSuccess::Ok)
    }
}
