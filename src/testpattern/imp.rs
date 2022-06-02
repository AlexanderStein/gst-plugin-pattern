// Copyright (C) 2022 Alexander Stein <alexander.stein@mailbox.org>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use gst::glib;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;
use std::sync::Mutex;

use once_cell::sync::Lazy;

// This module contains the private implementation details of our element
//
static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "rstestpattern",
        gst::DebugColorFlags::empty(),
        Some("Video Test Source"),
    )
});

// Default values of properties
const DEFAULT_FOREGROUND_COLOR: u32 = 0xffffffff;
const DEFAULT_BACKGROUND_COLOR: u32 = 0xff000000;
const DEFAULT_SPEED: u32 = 5;
const DEFAULT_SIZE: u32 = 50;

// Property value storage
#[derive(Debug, Clone)]
struct Settings {
    foreground_color: u32,
    background_color: u32,
    info: Option<gst_video::VideoInfo>,
    size: u32,
    offset: u32,
    speed: u32,

    accum_frames: u64,
    n_frames: u64,
    running_time: gst::ClockTime,
    accum_rtime: gst::ClockTime,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            foreground_color: DEFAULT_FOREGROUND_COLOR,
            background_color: DEFAULT_BACKGROUND_COLOR,
            speed: DEFAULT_SPEED,
            size: DEFAULT_SIZE,
            offset: 0,
            info: None,

            accum_frames: 0,
            n_frames: 0,
            running_time: gst::ClockTime::ZERO,
            accum_rtime: gst::ClockTime::ZERO,
        }
    }
}

// Struct containing all the element data
#[derive(Default)]
pub struct TestPatternSrc {
    settings: Mutex<Settings>,
}

impl TestPatternSrc {
    fn make_image(
        &self,
        _pts: gst::ClockTime,
        frame: &mut gst_video::VideoFrameRef<&mut gst::BufferRef>,
        settings: &mut Settings,
    ) {
        let info = settings.info.to_owned().unwrap();
        let stride = frame.plane_stride()[0] as usize;
        let width = frame.width() as usize * 4;

        let data = frame.plane_data_mut(0).unwrap();
        for (idx, line) in data.chunks_exact_mut(stride).enumerate() {
            for out_p in line[..width].chunks_exact_mut(4) {
                assert_eq!(out_p.len(), 4);
                let line_idx = idx as u32;

                if (line_idx >= settings.offset) && line_idx < (settings.offset + settings.size) {
                    out_p[0] = 0xff;
                    out_p[1] = 0xff;
                    out_p[2] = 0xff;
                } else {
                    out_p[0] = 0x00;
                    out_p[1] = 0x00;
                    out_p[2] = 0x00;
                }
            }
        }
        settings.offset += settings.speed;
        settings.offset %= info.height();
    }

    fn fill_image(
        &self,
        buffer: &mut gst::BufferRef,
        settings: &mut Settings,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let info = settings.info.to_owned().unwrap();

        if info.format() == gst_video::VideoFormat::Unknown {
            return Err(gst::FlowError::NotNegotiated);
        }
        let pts = buffer.pts().unwrap();
        match gst_video::VideoFrameRef::from_buffer_ref_writable(buffer, &info) {
            Err(_) => gst::debug!(CAT, "invalid frame"),
            Ok(mut frame) => self.make_image(pts, &mut frame, settings),
        }
        Ok(gst::FlowSuccess::Ok)
    }
}

// This trait registers our type with the GObject object system and
// provides the entry points for creating a new instance and setting
// up the class data
#[glib::object_subclass]
impl ObjectSubclass for TestPatternSrc {
    const NAME: &'static str = "TestPatternSrc";
    type Type = super::TestPatternSrc;
    type ParentType = gst_base::PushSrc;
}

// Implementation of glib::Object virtual methods
impl ObjectImpl for TestPatternSrc {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                glib::ParamSpecUInt::new(
                    "foreground-color",
                    "Foreground Color",
                    "Foreground color to use (big-endian ARGB)",
                    0,
                    u32::MAX,
                    DEFAULT_FOREGROUND_COLOR,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpecUInt::new(
                    "background-color",
                    "Background Color",
                    "Background color to use (big-endian ARGB)",
                    0,
                    u32::MAX,
                    DEFAULT_BACKGROUND_COLOR,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpecUInt::new(
                    "speed",
                    "Speed",
                    "Scroll image number of pixels per frame",
                    u32::MIN,
                    u32::MAX,
                    DEFAULT_SPEED,
                    glib::ParamFlags::READWRITE,
                ),
                glib::ParamSpecUInt::new(
                    "size",
                    "size",
                    "Vertical width of horizontal bar",
                    u32::MIN,
                    u32::MAX,
                    DEFAULT_SPEED,
                    glib::ParamFlags::READWRITE,
                ),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(
        &self,
        _obj: &Self::Type,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        let mut settings = self.settings.lock().unwrap();

        match pspec.name() {
            "foreground-color" => {
                settings.foreground_color = value.get().expect("type checked upstream");
            }
            "background-color" => {
                settings.background_color = value.get().expect("type checked upstream");
            }
            "speed" => {
                settings.speed = value.get().expect("type checked upstream");
            }
            "size" => {
                settings.size = value.get().expect("type checked upstream");
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        let settings = self.settings.lock().unwrap();
        match pspec.name() {
            "foreground-color" => {
                settings.foreground_color.to_value()
            }
            "background-color" => {
                settings.background_color.to_value()
            }
            "speed" => {
                settings.speed.to_value()
            }
            "size" => {
                settings.size.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        let mut settings = self.settings.lock().unwrap();
        settings.foreground_color = DEFAULT_FOREGROUND_COLOR;
        settings.background_color = DEFAULT_BACKGROUND_COLOR;
        settings.offset = 0;
        settings.size = DEFAULT_SIZE;
        settings.speed = DEFAULT_SPEED;

        // we operate in time
        obj.set_format(gst::Format::Time);
        obj.set_live(false);
    }
}

impl GstObjectImpl for TestPatternSrc {}

impl ElementImpl for TestPatternSrc {
    // Set the element specific metadata. This information is what
    // is visible from gst-inspect-1.0 and can also be programatically
    // retrieved from the gst::Registry after initial registration
    // without having to load the plugin in memory.
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "Video test source",
                "Source/Video",
                "Creates a test pattern video stream",
                "Alexander Stein <alexander.stein@mailbox.org>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps_raw = gst::Caps::builder("video/x-raw")
                .field(
                    "format",
                    gst_video::VideoFormat::Bgrx.to_str(),
                )
                .field("width", gst::IntRange::new(0, i32::MAX))
                .field("height", gst::IntRange::new(0, i32::MAX))
                .field(
                    "framerate",
                    gst::FractionRange::new(
                        gst::Fraction::new(0, 1),
                        gst::Fraction::new(i32::MAX, 1),
                    ),
                )
                .build();
            // The src pad template must be named "src" for pushsrc
            // and specific a pad that is always there
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps_raw,
            )
            .unwrap();

            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl BaseSrcImpl for TestPatternSrc {
    fn set_caps(&self, _element: &Self::Type, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        let mut settings = self.settings.lock().unwrap();
        let structure = caps.structure(0).unwrap();

        let info: gst_video::VideoInfo = if structure.name() == "video/x-raw" {
            gst_video::VideoInfo::from_caps(caps)?
        } else {
            return Err(gst::loggable_error!(CAT, "unsupported caps: {}", caps));
        };
        settings.info = Some(info);

        settings.accum_rtime = settings.running_time;
        settings.accum_frames = settings.n_frames;

        settings.running_time = gst::ClockTime::ZERO;
        settings.n_frames = 0;

        Ok(())
    }

    fn fixate(&self, element: &Self::Type, mut caps: gst::Caps) -> gst::Caps {
        let settings = self.settings.lock().unwrap();

        /* Check if foreground color has alpha, if it is the case,
         * force color format with an alpha channel downstream */
        if settings.foreground_color >> 24 != 255 {
            gst::loggable_error!(CAT, "foreground + alpha not (yet) supported");
            return caps;
        }
        drop(settings);

        {
            let caps = caps.make_mut();
            let s = caps.structure_mut(0).unwrap();
            s.fixate_field_nearest_int("width", 320);
            s.fixate_field_nearest_int("height", 240);

            if s.has_field("framerate") {
                s.fixate_field_nearest_fraction("framerate", gst::Fraction::new(30, 1));
            } else {
                s.set("framerate", gst::Fraction::new(30, 1));
            }

            // if s.has_field("pixel-aspect-ratio") {
            //     s.fixate_field_nearest_fraction("pixel-aspect-ratio", gst::Fraction::new(1, 1));
            // } else {
            //     s.set("pixel-aspect-ratio", gst::Fraction::new(1, 1));
            // }
        }

        self.parent_fixate(element, caps)
    }

    fn start(&self, _element: &Self::Type) -> Result<(), gst::ErrorMessage> {
        let mut settings = self.settings.lock().unwrap();
        settings.running_time = gst::ClockTime::ZERO;
        settings.n_frames = 0;
        settings.accum_frames = 0;
        settings.accum_rtime = gst::ClockTime::ZERO;

        let info = gst_video::VideoInfo::builder(gst_video::VideoFormat::Rgba, 320, 240)
            .views(1)
            .fps(gst::Fraction::new(0, 1))
            .par(gst::Fraction::new(0, 1))
            .multiview_mode(gst_video::VideoMultiviewMode::None)
            .field_order(gst_video::VideoFieldOrder::Unknown)
            .build()
            .unwrap();

        settings.info = Some(info);
        Ok(())
    }

    fn decide_allocation(
        &self,
        element: &Self::Type,
        query: &mut gst::query::Allocation,
    ) -> Result<(), gst::LoggableError> {
        let settings = self.settings.lock().unwrap();
        let info = settings.info.as_ref().unwrap();
        let pools = query.allocation_pools();

        let (pool, size, min, max, update) = if pools.is_empty() {
            (&None, info.size() as u32, 0, 0, false)
        } else {
            let (pool, pool_size, min, max) = &pools[0];
            (pool, info.size().max(*pool_size as usize) as u32, *min, *max, true)
        };

        let pool = if pool.is_none() {
            Some(gst_video::VideoBufferPool::new().upcast())
        } else {
            pool.clone()
        }.unwrap();

        let (caps, _) = query.get_owned();
        let mut config = pool.config();
        config.set_params(Some(&caps.copy()), size as u32, min, max);

        if query
            .find_allocation_meta::<gst_video::VideoMeta>()
            .is_some()
        {
            config.add_option(&gst_video::BUFFER_POOL_OPTION_VIDEO_META)
        }
        pool.set_config(config)?;
        if update {
            query.set_nth_allocation_pool(0, Some(&pool), size as u32, min, max);
        } else {
            query.add_allocation_pool(Some(&pool), size as u32, min, max);
        }

        self.parent_decide_allocation(element, query)
    }

    fn create(
        &self,
        element: &Self::Type,
        offset: u64,
        _buffer: Option<&mut gst::BufferRef>,
        length: u32,
    ) -> Result<CreateSuccess, gst::FlowError> {
        let mut buffer = BaseSrcImpl::alloc(self, element, offset, length)?;
        if length > 0 {
            BaseSrcImpl::fill(self, element, offset, length, buffer.make_mut())?;
        }
        Ok(CreateSuccess::NewBuffer(buffer))
    }

    fn alloc(
        &self,
        element: &Self::Type,
        _offset: u64,
        _length: u32,
    ) -> Result<gst::Buffer, gst::FlowError> {
        let settings = self.settings.lock().unwrap();
        let info = settings.info.to_owned().unwrap();
        match element.buffer_pool() {
            Some(pool) => pool.acquire_buffer(None),
            None => gst::Buffer::with_size((info.width() * info.height() * 4) as usize).map_err(|_| gst::FlowError::Error),
        }
    }
}

impl PushSrcImpl for TestPatternSrc {
    fn fill(
        &self,
        element: &Self::Type,
        buffer: &mut gst::BufferRef,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut settings = self.settings.lock().unwrap();
        let info = settings.info.to_owned().unwrap();
        let pts = settings.accum_rtime + settings.running_time;
        element.sync_values(pts).unwrap();

        buffer.set_pts(pts);
        self.fill_image(buffer, &mut settings)?;

        buffer.set_dts(gst::ClockTime::NONE);
        buffer.set_offset(settings.accum_frames + settings.n_frames);
        settings.n_frames += 1;
        buffer.set_offset_end(buffer.offset() + 1);

        let fps = info.fps();
                let next_time = unsafe {
            ffi::gst_util_uint64_scale(
                settings.n_frames,
                fps.denom() as u64 * gst::ClockTime::SECOND.nseconds(),
                fps.numer() as u64,
            )
        };
        let next_time = gst::ClockTime::from_nseconds(next_time);
        buffer.set_duration(next_time - settings.running_time);
        settings.running_time = next_time;

        Ok(gst::FlowSuccess::Ok)
    }
}
