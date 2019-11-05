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
    }

    glib_object_subclass!();
}

impl ObjectImpl for MyElement {
    glib_object_impl!();
}

impl ElementImpl for MyElement {}

impl BaseTransformImpl for MyElement {}
