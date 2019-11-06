use glib::glib_object_impl;
use glib::glib_object_subclass;
use glib::subclass::object::ObjectImpl;
use glib::subclass::simple::ClassStruct;
use glib::subclass::types::ObjectSubclass;
use gstreamer::gst_debug;
use gstreamer::gst_element_error;
use gstreamer::gst_info;
use gstreamer::subclass::element::ElementClassSubclassExt;
use gstreamer::subclass::element::ElementImpl;
use gstreamer::subclass::ElementInstanceStruct;
use gstreamer::Buffer;
use gstreamer::BufferRef;
use gstreamer::Caps;
use gstreamer::CapsIntersectMode;
use gstreamer::CoreError;
use gstreamer::DebugCategory;
use gstreamer::DebugColorFlags;
use gstreamer::ErrorMessage;
use gstreamer::FlowError;
use gstreamer::FlowSuccess;
use gstreamer::Fraction;
use gstreamer::FractionRange;
use gstreamer::IntRange;
use gstreamer::PadDirection;
use gstreamer::PadPresence;
use gstreamer::PadTemplate;
use gstreamer_base::subclass::base_transform::BaseTransformClassSubclassExt;
use gstreamer_base::subclass::base_transform::BaseTransformImpl;
use gstreamer_base::subclass::BaseTransformMode::NeverInPlace;
use gstreamer_base::BaseTransform;
use gstreamer_video::VideoFormat;
use gstreamer_video::VideoFrameRef;
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
                ("format", &VideoFormat::Bgrx.to_string()),
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

    fn transform(
        &self,
        element: &BaseTransform,
        inbuf: &Buffer,
        outbuf: &mut BufferRef,
    ) -> Result<FlowSuccess, FlowError> {
        let mut state_guard = self.state.lock().unwrap();
        let state = state_guard.as_mut().ok_or_else(|| {
            gst_element_error!(element, CoreError::Negotiation, ["Have no state yet"]);
            FlowError::NotNegotiated
        })?;

        let in_frame = VideoFrameRef::from_buffer_ref_readable(inbuf.as_ref(), &state.in_info)
            .ok_or_else(|| {
                gst_element_error!(
                    element,
                    CoreError::Failed,
                    ["Failed to map input buffer readable"]
                );
                FlowError::Error
            })?;

        let mut out_frame = VideoFrameRef::from_buffer_ref_writable(outbuf, &state.out_info)
            .ok_or_else(|| {
                gst_element_error!(
                    element,
                    CoreError::Failed,
                    ["Failed to map output buffer writable"]
                );
                FlowError::Error
            })?;

        let width = in_frame.width() as usize;
        let in_stride = in_frame.plane_stride()[0] as usize;
        let in_data = in_frame.plane_data(0).unwrap();
        let out_stride = out_frame.plane_stride()[0] as usize;
        let out_format = out_frame.format();
        let out_data = out_frame.plane_data_mut(0).unwrap();

        if out_format == VideoFormat::Bgrx {
            assert_eq!(in_data.len() % 4, 0);
            assert_eq!(out_data.len() % 4, 0);
            assert_eq!(out_data.len() / out_stride, in_data.len() / in_stride);

            let in_line_bytes = width * 4;
            let out_line_bytes = width * 4;

            assert!(in_line_bytes <= in_stride);
            assert!(out_line_bytes <= out_stride);

            for (in_line, out_line) in in_data
                .chunks_exact(in_stride)
                .zip(out_data.chunks_exact_mut(out_stride))
            {
                for (in_p, out_p) in in_line[..in_line_bytes]
                    .chunks_exact(4)
                    .zip(out_line[..out_line_bytes].chunks_exact_mut(4))
                {
                    assert_eq!(out_p.len(), 4);

                    out_p[0] = in_p[0] / 2;
                    out_p[1] = in_p[1] / 2 + 127;
                    out_p[2] = in_p[2] / 2;
                    out_p[3] = in_p[3];
                }
            }
        } else {
            unimplemented!();
        }

        Ok(FlowSuccess::Ok)
    }
}
