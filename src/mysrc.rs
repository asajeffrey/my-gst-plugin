use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::subclass::object::ObjectImpl;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
use gstreamer::subclass::element::ElementImpl;
use gstreamer::subclass::ElementInstanceStruct;
use gstreamer_base::subclass::base_src::BaseSrcImpl;
use gstreamer_base::BaseSrc;

pub struct MySrc {}

impl ObjectSubclass for MySrc {
    const NAME: &'static str = "MySrc";
    type ParentType = BaseSrc;
    type Instance = ElementInstanceStruct<Self>;
    type Class = ClassStruct<Self>;

    fn new() -> Self {
        Self {}
    }

    glib_object_subclass!();
}

impl ObjectImpl for MySrc {
    glib_object_impl!();
}

impl ElementImpl for MySrc {}

impl BaseSrcImpl for MySrc {}
