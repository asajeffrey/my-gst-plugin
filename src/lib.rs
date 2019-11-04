use gstreamer::gst_plugin_define;

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

fn plugin_init(_plugin: &gstreamer::Plugin) -> Result<(), glib::BoolError> {
    Ok(())
}
