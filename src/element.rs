use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::subclass::object::ObjectImpl;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
use gstreamer::gst_debug;
use gstreamer::gst_info;
use gstreamer::subclass::element::ElementClassSubclassExt;
use gstreamer::subclass::element::ElementImpl;
use gstreamer::subclass::ElementInstanceStruct;
use gstreamer::Caps;
use gstreamer::CapsIntersectMode;
use gstreamer::DebugCategory;
use gstreamer::DebugColorFlags;
use gstreamer::ErrorMessage;
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
use gstreamer_video::VideoInfo;

use std::sync::Mutex;

struct State {
    in_info: VideoInfo,
    out_info: VideoInfo,
}

pub struct MyElement {
    cat: DebugCategory,
    state: Mutex<Option<State>>,
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
            state: Mutex::new(None),
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

impl BaseTransformImpl for MyElement {
    fn set_caps(&self, element: &BaseTransform, incaps: &Caps, outcaps: &Caps) -> bool {
        let in_info = match VideoInfo::from_caps(incaps) {
            None => return false,
            Some(info) => info,
        };
        let out_info = match VideoInfo::from_caps(outcaps) {
            None => return false,
            Some(info) => info,
        };
        gst_debug!(
            self.cat,
            obj: element,
            "Configured for caps {} to {}",
            incaps,
            outcaps
        );
        *self.state.lock().unwrap() = Some(State { in_info, out_info });
        true
    }

    fn stop(&self, element: &BaseTransform) -> Result<(), ErrorMessage> {
        let _ = self.state.lock().unwrap().take();
        gst_info!(self.cat, obj: element, "Stopped");
        Ok(())
    }

    fn get_unit_size(&self, _element: &BaseTransform, caps: &Caps) -> Option<usize> {
        VideoInfo::from_caps(caps).map(|info| info.size())
    }

    fn transform_caps(
        &self,
        element: &BaseTransform,
        direction: PadDirection,
        caps: &Caps,
        filter: Option<&Caps>,
    ) -> Option<Caps> {
        let other_caps = if direction == PadDirection::Src {
            let mut caps = caps.clone();

            for s in caps.make_mut().iter_mut() {
                s.set("format", &VideoFormat::Bgrx.to_string());
            }

            caps
        } else {
            let mut gray_caps = Caps::new_empty();

            {
                let gray_caps = gray_caps.get_mut().unwrap();

                for s in caps.iter() {
                    let mut s_gray = s.to_owned();
                    s_gray.set("format", &VideoFormat::Gray8.to_string());
                    gray_caps.append_structure(s_gray);
                }
                gray_caps.append(caps.clone());
            }

            gray_caps
        };

        gst_debug!(
            self.cat,
            obj: element,
            "Transformed caps from {} to {} in direction {:?}",
            caps,
            other_caps,
            direction
        );

        if let Some(filter) = filter {
            Some(filter.intersect_with_mode(&other_caps, CapsIntersectMode::First))
        } else {
            Some(other_caps)
        }
    }
}
