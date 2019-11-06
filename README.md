# my-gst-plugin

Playground for a GST plugin

Following along with [How to write GStreamer Elements in Rust Part 1](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/blob/master/gst-plugin-tutorial/tutorial-1.md) by [Sebastian Dröge](https://coaxion.net/).

Build with `cargo build --release`

Run with `GST_PLUGIN_PATH=target/release gst-launch-1.0 videotestsrc ! mytransform ! videoconvert ! autovideosink`

![Sickly green test card](https://user-images.githubusercontent.com/403333/68335528-3dd08880-00a2-11ea-8ff2-5a63858b81e3.png)

or with `GST_PLUGIN_PATH=target/release gst-launch-1.0 playbin uri=https://download.blender.org/peach/trailer/trailer_400p.ogg video-filter=mytransform`

![Sickly green Big Buck Bunny](https://user-images.githubusercontent.com/403333/68335440-14176180-00a2-11ea-8c42-766692bcf3bb.png)
