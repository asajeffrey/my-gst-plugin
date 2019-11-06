use glib::glib_bool_error;
use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::subclass::object::ObjectImpl;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
use gstreamer::gst_debug;
use gstreamer::gst_loggable_error;
use gstreamer::subclass::element::ElementClassSubclassExt;
use gstreamer::subclass::element::ElementImpl;
use gstreamer::subclass::ElementInstanceStruct;
use gstreamer::Caps;
use gstreamer::DebugCategory;
use gstreamer::DebugColorFlags;
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
use gstreamer_video::VideoInfo;

use std::sync::Mutex;

pub struct MySrc {
    cat: DebugCategory,
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

        let sink_caps = Caps::new_simple(
            "video/x-raw",
            &[
                ("format", &VideoFormat::Bgrx.to_string()),
                ("width", &IntRange::<i32>::new(0, std::i32::MAX)),
                ("height", &IntRange::<i32>::new(0, std::i32::MAX)),
                (
                    "framerate",
                    &FractionRange::new(Fraction::new(0, 1), Fraction::new(std::i32::MAX, 1)),
                ),
            ],
        );
        let sink_pad_template =
            PadTemplate::new("sink", PadDirection::Sink, PadPresence::Always, &sink_caps).unwrap();
        klass.add_pad_template(sink_pad_template);
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
}
