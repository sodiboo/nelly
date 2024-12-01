use std::sync::atomic::{AtomicU32, Ordering};

/// A counter for generating serials, for use in the client protocol
///
/// A global instance of this counter is available as the `SERIAL_COUNTER`
/// static. It is recommended to only use this global counter to ensure the
/// uniqueness of serials.
///
/// The counter will wrap around on overflow, ensuring it can run for as long
/// as needed.
#[derive(Debug)]
pub struct SerialCounter {
    serial: AtomicU32,
}

impl Default for SerialCounter {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl SerialCounter {
    /// Create a new counter starting at `1`
    pub const fn new() -> Self {
        Self {
            serial: AtomicU32::new(1),
        }
    }

    /// Retrieve the next serial from the counter
    pub fn next_serial(&self) -> u32 {
        let _ = self
            .serial
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::SeqCst);
        self.serial.fetch_add(1, Ordering::AcqRel)
    }
}

/// State of key on a keyboard. Either pressed or released
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum KeyState {
    /// Key is released
    Released,
    /// Key is pressed
    Pressed,
}

/// State of a button on a pointer device, like mouse or tablet tool. Either pressed or released
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ButtonState {
    /// Button is released
    Released,
    /// Button is pressed
    Pressed,
}

/// Axis when scrolling
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Axis {
    /// Vertical axis
    Vertical,
    /// Horizontal axis
    Horizontal,
}

/// Source of an axis when scrolling
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AxisSource {
    /// Finger. Mostly used for trackpads.
    ///
    /// Guarantees that a scroll sequence is terminated with a scroll value of 0.
    /// A caller may use this information to decide on whether kinetic scrolling should
    /// be triggered on this scroll sequence.
    ///
    /// The coordinate system is identical to the
    /// cursor movement, i.e. a scroll value of 1 represents the equivalent relative
    /// motion of 1.
    Finger,
    /// Continuous scrolling device. Almost identical to [`Finger`](AxisSource::Finger)
    ///
    /// No terminating event is guaranteed (though it may happen).
    ///
    /// The coordinate system is identical to
    /// the cursor movement, i.e. a scroll value of 1 represents the equivalent relative
    /// motion of 1.
    Continuous,
    /// Scroll wheel.
    ///
    /// No terminating event is guaranteed (though it may happen). Scrolling is in
    /// discrete steps. It is up to the caller how to interpret such different step sizes.
    Wheel,
    /// Scrolling through tilting the scroll wheel.
    ///
    /// No terminating event is guaranteed (though it may happen). Scrolling is in
    /// discrete steps. It is up to the caller how to interpret such different step sizes.
    WheelTilt,
}

/// Direction of physical motion that caused pointer axis event
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AxisRelativeDirection {
    /// Physical motion matches axis direction
    Identical,
    /// Physical motion is inverse of axis direction (e.g. natural scrolling)
    Inverted,
}

#[derive(Debug, PartialEq, Clone)]
pub struct AxisFrame {
    pub time: u32,
    pub horizontal: AxisScroll,
    pub vertical: AxisScroll,
    pub source: AxisSource,
}

#[derive(Debug, PartialEq, Clone)]
pub struct AxisScroll {
    pub absolute: f64,
    pub v120: i32,
    pub relative_direction: AxisRelativeDirection,
}

impl std::ops::Index<Axis> for AxisFrame {
    type Output = AxisScroll;

    fn index(&self, axis: Axis) -> &Self::Output {
        match axis {
            Axis::Vertical => &self.vertical,
            Axis::Horizontal => &self.horizontal,
        }
    }
}

impl std::ops::IndexMut<Axis> for AxisFrame {
    fn index_mut(&mut self, axis: Axis) -> &mut Self::Output {
        match axis {
            Axis::Vertical => &mut self.vertical,
            Axis::Horizontal => &mut self.horizontal,
        }
    }
}

impl Default for AxisFrame {
    fn default() -> Self {
        AxisFrame {
            time: 0, // Should always be overwritten.
            horizontal: AxisScroll {
                absolute: 0.0,
                v120: 0,
                relative_direction: AxisRelativeDirection::Identical,
            },
            vertical: AxisScroll {
                absolute: 0.0,
                v120: 0,
                relative_direction: AxisRelativeDirection::Identical,
            },
            // I assume most compositors always send an axis source (we certainly do in niri).
            // As such, this "should" always be overwritten. If it isn't, it's probably a bug,
            // But maybe the compositor doesn't support v5 of the wl_pointer protocol at all.
            // In that case, we know we won't get any axis_source, and i think for such an old
            // compositor we're most likely to be dealing with a Wheel.
            source: AxisSource::Wheel,
        }
    }
}

impl AxisFrame {
    pub fn time(&mut self, time: u32) {
        if self.time == 0 {
            self.time = time;
        }
    }
}
