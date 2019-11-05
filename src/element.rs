use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::subclass::object::ObjectImpl;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
use gstreamer::subclass::element::ElementClassSubclassExt;
use gstreamer::subclass::element::ElementImpl;
use gstreamer::subclass::ElementInstanceStruct;
use gstreamer::Caps;
use gstreamer::DebugCategory;
use gstreamer::DebugColorFlags;
use gstreamer::Fraction;
use gstreamer::FractionRange;
use gstreamer::IntRange;
use gstreamer::List;
use gstreamer::PadDirection;
use gstreamer::PadPresence;
use gstreamer::PadTemplate;
use gstreamer_base::subclass::base_transform::BaseTransformClassSubclassExt;
use gstreamer_base::subclass::base_transform::BaseTransformImpl;
use gstreamer_base::subclass::BaseTransformMode::NeverInPlace;
use gstreamer_base::BaseTransform;
use gstreamer_video::VideoFormat;

pub struct MyElement {
    cat: DebugCategory,
}

impl ObjectSubclass for MyElement {
    const NAME: &'static str = "MyElement";
    type ParentType = BaseTransform;
    type Instance = ElementInstanceStruct<Self>;
    type Class = ClassStruct<Self>;

    fn new() -> Self {
        Self {
            cat: DebugCategory::new(
                "myelement",
                DebugColorFlags::empty(),
                Some("My element by me"),
            ),
        }
    }

    fn class_init(klass: &mut ClassStruct<Self>) {
        klass.set_metadata(
            "My Element By Me",
            "Filter/Effect/Converter/Video",
            "Does stuff",
            env!("CARGO_PKG_AUTHORS"),
        );

        klass.configure(NeverInPlace, false, false);

        let src_caps = Caps::new_simple(
            "video/x-raw",
            &[
                (
                    "format",
                    &List::new(&[
                        &VideoFormat::Bgrx.to_string(),
                        &VideoFormat::Gray8.to_string(),
                    ]),
                ),
                ("width", &IntRange::<i32>::new(0, std::i32::MAX)),
                ("height", &IntRange::<i32>::new(0, std::i32::MAX)),
                (
                    "framerate",
                    &FractionRange::new(Fraction::new(0, 1), Fraction::new(std::i32::MAX, 1)),
                ),
            ],
        );
        let src_pad_template =
            PadTemplate::new("src", PadDirection::Src, PadPresence::Always, &src_caps).unwrap();
        klass.add_pad_template(src_pad_template);

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

impl ObjectImpl for MyElement {
    glib_object_impl!();
}

impl ElementImpl for MyElement {}

impl BaseTransformImpl for MyElement {}
