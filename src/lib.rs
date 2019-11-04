use element::MyElement;
use glib::subclass::types::ObjectSubclass;
use gstreamer::gst_plugin_define;

mod element;

gst_plugin_define!(
    myplugin,
    env!("CARGO_PKG_DESCRIPTION"),
    plugin_init,
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")),
    "MPL",
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_NAME"),
    env!("CARGO_PKG_REPOSITORY"),
    env!("BUILD_REL_DATE")
);

fn plugin_init(plugin: &gstreamer::Plugin) -> Result<(), glib::BoolError> {
    gstreamer::Element::register(
        Some(plugin),
        "myelement",
        gstreamer::Rank::None,
        MyElement::get_type(),
    )
}
