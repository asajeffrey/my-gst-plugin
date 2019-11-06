use glib::subclass::types::ObjectSubclass;
use gstreamer::gst_plugin_define;
use mysrc::MySrc;
use mytransform::MyTransform;

mod mysrc;
mod mytransform;

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
        "mytransform",
        gstreamer::Rank::None,
        MyTransform::get_type(),
    )?;
    gstreamer::Element::register(
        Some(plugin),
        "mysrc",
        gstreamer::Rank::None,
        MySrc::get_type(),
    )?;
    Ok(())
}
