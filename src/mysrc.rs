use glib::glib_bool_error;
use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::subclass::object::ObjectImpl;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
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

use std::sync::Mutex;
use std::time::Instant;

pub struct MySrc {
    cat: DebugCategory,
    start: Instant,
    out_info: Mutex<Option<VideoInfo>>,
}

impl ObjectSubclass for MySrc {
    const NAME: &'static str = "MySrc";
    type ParentType = BaseSrc;
    type Instance = ElementInstanceStruct<Self>;
    type Class = ClassStruct<Self>;

    fn new() -> Self {
        Self {
            cat: DebugCategory::new("mysrc", DebugColorFlags::empty(), Some("My src by me")),
            start: Instant::now(),
            out_info: Mutex::new(None),
        }
    }

    fn class_init(klass: &mut ClassStruct<Self>) {
        klass.set_metadata(
            "My Src By Me",
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

impl ObjectImpl for MySrc {
    glib_object_impl!();
}

impl ElementImpl for MySrc {}

impl BaseSrcImpl for MySrc {
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
        let out_guard = self.out_info.lock().map_err(|_| {
            gst_element_error!(src, CoreError::Negotiation, ["Lock poisoned"]);
            FlowError::NotNegotiated
        })?;
        let out_info = out_guard.as_ref().ok_or_else(|| {
            gst_element_error!(src, CoreError::Negotiation, ["Caps not set yet"]);
            FlowError::NotNegotiated
        })?;
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
        let data = out_frame.plane_data_mut(0).unwrap();
        gst_debug!(
            self.cat,
            obj: src,
            "Filling mysrc buffer {}x{} {:?}",
            width,
            height,
            format
        );

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

        Ok(FlowSuccess::Ok)
    }
}
