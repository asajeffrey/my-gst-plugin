use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::subclass::object::ObjectImpl;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
use gstreamer::subclass::element::ElementClassSubclassExt;
use gstreamer::subclass::element::ElementImpl;
use gstreamer::subclass::ElementInstanceStruct;
use gstreamer_base::subclass::base_transform::BaseTransformClassSubclassExt;
use gstreamer_base::subclass::base_transform::BaseTransformImpl;
use gstreamer_base::subclass::BaseTransformMode::NeverInPlace;
use gstreamer_base::BaseTransform;

pub struct MyElement {}

impl ObjectSubclass for MyElement {
    const NAME: &'static str = "MyElement";
    type ParentType = BaseTransform;
    type Instance = ElementInstanceStruct<Self>;
    type Class = ClassStruct<Self>;

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
