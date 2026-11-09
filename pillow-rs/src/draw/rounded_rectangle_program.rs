use super::{
    RoundedRectangleBounds, RoundedRectangleColor, RoundedRectangleOutlineBorder,
    RoundedRectangleOutlineCallbacks,
};
use crate::error::PilError;
use std::collections::VecDeque;

/// Numeric callbacks used by the rounded-rectangle program at each Pillow
/// sequence point.
#[doc(hidden)]
pub trait RoundedRectangleProgramCallbacks: RoundedRectangleOutlineCallbacks {}

impl<T: RoundedRectangleOutlineCallbacks> RoundedRectangleProgramCallbacks for T {}

/// One native primitive emitted by Pillow's rounded-rectangle program.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum RoundedRectangleDrawStep<Value> {
    /// Draw one filled corner sector.
    PieSlice {
        /// Pillow-style floating-point bounds.
        bounds: RoundedRectangleBounds<Value>,
        /// Start angle in degrees.
        start: f64,
        /// End angle in degrees.
        end: f64,
        /// Resolved native fill color.
        color: RoundedRectangleColor,
    },
    /// Draw one outline arc.
    Arc {
        /// Pillow-style floating-point bounds.
        bounds: RoundedRectangleBounds<Value>,
        /// Start angle in degrees.
        start: f64,
        /// End angle in degrees.
        end: f64,
        /// Resolved native outline color.
        color: RoundedRectangleColor,
    },
    /// Draw one filled rectangular region.
    Rectangle {
        /// Pillow-style floating-point bounds.
        bounds: RoundedRectangleBounds<Value>,
        /// Resolved native fill color.
        color: RoundedRectangleColor,
    },
}

/// Ordered host interaction requested by a rounded-rectangle program.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum RoundedRectangleProgramStep<Value> {
    /// Read and truth-test the selected item from the caller's corner object.
    ReadCorner(usize),
    /// Compare the original outline width with zero after fill drawing.
    ReadOutlineEnabled,
    /// Supply the original width for the next outline arc.
    ReadWidth,
    /// Request a border's host numeric callbacks; core builds its bounds.
    OutlineBorder {
        /// Border rectangle to construct.
        kind: RoundedRectangleOutlineBorder,
        /// Original rounded-rectangle coordinates.
        bounds: RoundedRectangleBounds<Value>,
        /// Floored corner radius used for corner offsets.
        radius: Value,
        /// Resolved native outline color.
        color: RoundedRectangleColor,
    },
    /// Execute one native primitive before asking for the next interaction.
    Draw(RoundedRectangleDrawStep<Value>),
    /// The program has no remaining work.
    Done,
}

/// Host value returned to a rounded-rectangle program request.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundedRectangleProgramResponse {
    /// Result of truth-testing one indexed corner.
    Corner(bool),
    /// Result of Pillow's `width != 0` check.
    OutlineEnabled(bool),
    /// The caller supplied the original width for the next native arc call.
    Width,
}

/// Core state-machine failure versus a Python numeric protocol failure.
#[doc(hidden)]
pub enum RoundedRectangleProgramError<Error> {
    /// Invalid or incomplete host response to a core request.
    Core(PilError),
    /// Failure raised while running one Python numeric protocol.
    Callback(Error),
}

impl<Error> From<Error> for RoundedRectangleProgramError<Error> {
    fn from(error: Error) -> Self {
        Self::Callback(error)
    }
}

#[derive(Clone, Debug)]
enum Instruction<Value> {
    Curves {
        bounds: RoundedRectangleBounds<Value>,
        diameter: Value,
        full_x: bool,
        full_y: bool,
        color: RoundedRectangleColor,
        outline: bool,
    },
    FillMainRectangle {
        bounds: RoundedRectangleBounds<Value>,
        radius: Value,
        full_x: bool,
        color: RoundedRectangleColor,
    },
    FillCornerRectangle {
        kind: FillCornerRectangle,
        bounds: RoundedRectangleBounds<Value>,
        radius: Value,
        color: RoundedRectangleColor,
    },
    OutlineBorder(RoundedRectangleOutlineBorder, OutlineContext<Value>),
    ReadOutlineEnabled,
}

/// Core-owned outline geometry held until Pillow's width comparison succeeds.
#[derive(Clone, Debug)]
struct OutlineContext<Value> {
    bounds: RoundedRectangleBounds<Value>,
    diameter: Value,
    radius: Value,
    full_x: bool,
    full_y: bool,
    color: RoundedRectangleColor,
}

#[derive(Clone, Debug)]
enum FillCornerRectangle {
    Left,
    Right,
}

#[derive(Clone, Debug)]
enum Event<Value> {
    ReadCorner(usize),
    DrawIfCorner(usize, RoundedRectangleDrawStep<Value>),
    Draw(RoundedRectangleDrawStep<Value>),
    AdjustCorner {
        corner: usize,
        coordinate: usize,
        add: bool,
        radius: Value,
    },
    EmitAdjustedRectangle,
}

#[derive(Clone, Debug)]
struct AdjustedRectangle<Value> {
    bounds: RoundedRectangleBounds<Value>,
    color: RoundedRectangleColor,
}

#[derive(Clone, Debug)]
enum WidthRequest<Value> {
    Arc(RoundedRectangleDrawStep<Value>),
}

#[derive(Clone, Debug)]
enum ProgramRequest<Value> {
    Corner(usize),
    OutlineEnabled,
    Width(WidthRequest<Value>),
}

/// Core-owned rounded-rectangle geometry with host reads at Pillow's exact
/// sequence points. The adapter applies each emitted primitive before asking
/// for the next step so callbacks can observe prior drawing mutations.
#[doc(hidden)]
pub struct RoundedRectangleProgram<Value> {
    instructions: VecDeque<Instruction<Value>>,
    events: VecDeque<Event<Value>>,
    outline: Option<OutlineContext<Value>>,
    corners: [bool; 4],
    waiting: Option<ProgramRequest<Value>>,
    pending_draw: Option<RoundedRectangleDrawStep<Value>>,
    adjusted_rectangle: Option<AdjustedRectangle<Value>>,
}

impl<Value: Clone> RoundedRectangleProgram<Value> {
    /// Build an ordered program from Pillow-prepared values and native inks.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        coordinates: RoundedRectangleBounds<Value>,
        diameter: Value,
        radius: Value,
        full_x: bool,
        full_y: bool,
        fill: Option<RoundedRectangleColor>,
        outline: Option<RoundedRectangleColor>,
        fill_ink: Option<i32>,
        outline_ink: Option<i32>,
    ) -> Self {
        let mut instructions = VecDeque::new();
        if let Some(color) = fill {
            push_curve_instruction(
                &mut instructions,
                coordinates.clone(),
                diameter.clone(),
                full_x,
                full_y,
                color,
                false,
            );
            instructions.push_back(Instruction::FillMainRectangle {
                bounds: coordinates.clone(),
                radius: radius.clone(),
                full_x,
                color,
            });
            if !full_x && !full_y {
                instructions.push_back(Instruction::FillCornerRectangle {
                    kind: FillCornerRectangle::Left,
                    bounds: coordinates.clone(),
                    radius: radius.clone(),
                    color,
                });
                instructions.push_back(Instruction::FillCornerRectangle {
                    kind: FillCornerRectangle::Right,
                    bounds: coordinates.clone(),
                    radius: radius.clone(),
                    color,
                });
            }
        }
        let outline = outline.filter(|_| outline_ink.is_some() && outline_ink != fill_ink);
        let outline = outline.map(|color| OutlineContext {
            bounds: coordinates,
            diameter,
            radius,
            full_x,
            full_y,
            color,
        });
        if outline.is_some() {
            instructions.push_back(Instruction::ReadOutlineEnabled);
        }
        Self {
            instructions,
            events: VecDeque::new(),
            outline,
            corners: [false; 4],
            waiting: None,
            pending_draw: None,
            adjusted_rectangle: None,
        }
    }

    /// Advance until the next host read, draw operation, or completion.
    pub fn next_step<C: RoundedRectangleProgramCallbacks<Value = Value>>(
        &mut self,
        response: Option<RoundedRectangleProgramResponse>,
        callbacks: &mut C,
    ) -> Result<RoundedRectangleProgramStep<Value>, RoundedRectangleProgramError<C::Error>> {
        self.accept_response(response)?;
        if let Some(step) = self.pending_draw.take() {
            return Ok(RoundedRectangleProgramStep::Draw(step));
        }
        loop {
            if let Some(event) = self.events.pop_front() {
                if let Some(step) = self.process_event(event, callbacks)? {
                    return Ok(step);
                }
                continue;
            }
            let Some(instruction) = self.instructions.pop_front() else {
                return Ok(RoundedRectangleProgramStep::Done);
            };
            if let Some(step) = self.expand_instruction(instruction, callbacks)? {
                return Ok(step);
            }
        }
    }

    fn draw_or_request_width(
        &mut self,
        step: RoundedRectangleDrawStep<Value>,
    ) -> RoundedRectangleProgramStep<Value> {
        if matches!(&step, RoundedRectangleDrawStep::Arc { .. }) {
            self.waiting = Some(ProgramRequest::Width(WidthRequest::Arc(step)));
            RoundedRectangleProgramStep::ReadWidth
        } else {
            RoundedRectangleProgramStep::Draw(step)
        }
    }

    fn process_event<C: RoundedRectangleProgramCallbacks<Value = Value>>(
        &mut self,
        event: Event<Value>,
        callbacks: &mut C,
    ) -> Result<Option<RoundedRectangleProgramStep<Value>>, RoundedRectangleProgramError<C::Error>>
    {
        match event {
            Event::ReadCorner(index) => {
                self.waiting = Some(ProgramRequest::Corner(index));
                Ok(Some(RoundedRectangleProgramStep::ReadCorner(index)))
            }
            Event::Draw(step) => Ok(Some(self.draw_or_request_width(step))),
            Event::DrawIfCorner(index, step) if self.corners[index] => {
                Ok(Some(self.draw_or_request_width(step)))
            }
            Event::DrawIfCorner(_, _) => Ok(None),
            Event::AdjustCorner {
                corner,
                coordinate,
                add,
                radius,
            } => {
                self.adjust_corner(corner, coordinate, add, &radius, callbacks)?;
                Ok(None)
            }
            Event::EmitAdjustedRectangle => {
                let rectangle = self.adjusted_rectangle.take().ok_or_else(|| {
                    RoundedRectangleProgramError::Core(PilError::InternalError(
                        "rounded rectangle adjustment has no rectangle".into(),
                    ))
                })?;
                let step = RoundedRectangleDrawStep::Rectangle {
                    bounds: rectangle.bounds,
                    color: rectangle.color,
                };
                Ok(Some(RoundedRectangleProgramStep::Draw(step)))
            }
        }
    }

    fn expand_instruction<C: RoundedRectangleProgramCallbacks<Value = Value>>(
        &mut self,
        instruction: Instruction<Value>,
        callbacks: &mut C,
    ) -> Result<Option<RoundedRectangleProgramStep<Value>>, RoundedRectangleProgramError<C::Error>>
    {
        match instruction {
            Instruction::Curves {
                bounds,
                diameter,
                full_x,
                full_y,
                color,
                outline,
            } => {
                self.expand_curves(bounds, diameter, full_x, full_y, color, outline, callbacks)?;
                Ok(None)
            }
            Instruction::FillMainRectangle {
                bounds,
                radius,
                full_x,
                color,
            } => {
                self.expand_fill_main_rectangle(bounds, radius, full_x, color, callbacks)?;
                Ok(None)
            }
            Instruction::FillCornerRectangle {
                kind,
                bounds,
                radius,
                color,
            } => {
                self.expand_fill_corner_rectangle(kind, bounds, radius, color, callbacks)?;
                Ok(None)
            }
            Instruction::OutlineBorder(kind, outline) => {
                Ok(Some(RoundedRectangleProgramStep::OutlineBorder {
                    kind,
                    bounds: outline.bounds,
                    radius: outline.radius,
                    color: outline.color,
                }))
            }
            Instruction::ReadOutlineEnabled => {
                self.waiting = Some(ProgramRequest::OutlineEnabled);
                Ok(Some(RoundedRectangleProgramStep::ReadOutlineEnabled))
            }
        }
    }

    fn expand_curves<C: RoundedRectangleProgramCallbacks<Value = Value>>(
        &mut self,
        bounds: RoundedRectangleBounds<Value>,
        diameter: Value,
        full_x: bool,
        full_y: bool,
        color: RoundedRectangleColor,
        outline: bool,
        callbacks: &mut C,
    ) -> Result<(), RoundedRectangleProgramError<C::Error>> {
        let parts = rounded_curve_parts(&bounds, &diameter, full_x, full_y, callbacks)
            .map_err(RoundedRectangleProgramError::Callback)?;
        if !full_x && !full_y {
            for index in 0..4 {
                self.events.push_back(Event::ReadCorner(index));
            }
        }
        for (index, (bounds, start, end)) in parts.into_iter().enumerate() {
            let step = if outline {
                RoundedRectangleDrawStep::Arc {
                    bounds,
                    start,
                    end,
                    color,
                }
            } else {
                RoundedRectangleDrawStep::PieSlice {
                    bounds,
                    start,
                    end,
                    color,
                }
            };
            self.events.push_back(if full_x || full_y {
                Event::Draw(step)
            } else {
                Event::DrawIfCorner(index, step)
            });
        }
        Ok(())
    }

    fn expand_fill_main_rectangle<C: RoundedRectangleProgramCallbacks<Value = Value>>(
        &mut self,
        bounds: RoundedRectangleBounds<Value>,
        radius: Value,
        full_x: bool,
        color: RoundedRectangleColor,
        callbacks: &mut C,
    ) -> Result<(), RoundedRectangleProgramError<C::Error>> {
        let rectangle = if full_x {
            Some([
                bounds[0].clone(),
                add_one(&bounds[1], &radius, callbacks)?,
                bounds[2].clone(),
                subtract_one(&bounds[3], &radius, callbacks)?,
            ])
        } else {
            let left = subtract_one(&bounds[2], &radius, callbacks)?;
            let right = add_one(&bounds[0], &radius, callbacks)?;
            if callbacks
                .greater_equal(&left, &right)
                .map_err(RoundedRectangleProgramError::Callback)?
            {
                Some([
                    add_one(&bounds[0], &radius, callbacks)?,
                    bounds[1].clone(),
                    subtract_one(&bounds[2], &radius, callbacks)?,
                    bounds[3].clone(),
                ])
            } else {
                None
            }
        };
        if let Some(bounds) = rectangle {
            self.events
                .push_back(Event::Draw(RoundedRectangleDrawStep::Rectangle {
                    bounds,
                    color,
                }));
        }
        Ok(())
    }

    fn expand_fill_corner_rectangle<C: RoundedRectangleProgramCallbacks<Value = Value>>(
        &mut self,
        kind: FillCornerRectangle,
        bounds: RoundedRectangleBounds<Value>,
        radius: Value,
        color: RoundedRectangleColor,
        callbacks: &mut C,
    ) -> Result<(), RoundedRectangleProgramError<C::Error>> {
        let initial = match kind {
            FillCornerRectangle::Left => [
                bounds[0].clone(),
                bounds[1].clone(),
                callbacks
                    .add(&bounds[0], &radius)
                    .map_err(RoundedRectangleProgramError::Callback)?,
                bounds[3].clone(),
            ],
            FillCornerRectangle::Right => [
                callbacks
                    .subtract(&bounds[2], &radius)
                    .map_err(RoundedRectangleProgramError::Callback)?,
                bounds[1].clone(),
                bounds[2].clone(),
                bounds[3].clone(),
            ],
        };
        self.adjusted_rectangle = Some(AdjustedRectangle {
            bounds: initial,
            color,
        });
        match kind {
            FillCornerRectangle::Left => {
                self.queue_corner_adjustment(0, 1, true, &radius);
                self.queue_corner_adjustment(3, 3, false, &radius);
            }
            FillCornerRectangle::Right => {
                self.queue_corner_adjustment(1, 1, true, &radius);
                self.queue_corner_adjustment(2, 3, false, &radius);
            }
        }
        self.events.push_back(Event::EmitAdjustedRectangle);
        Ok(())
    }

    fn queue_corner_adjustment(
        &mut self,
        corner: usize,
        coordinate: usize,
        add: bool,
        radius: &Value,
    ) {
        self.events.push_back(Event::ReadCorner(corner));
        self.events.push_back(Event::AdjustCorner {
            corner,
            coordinate,
            add,
            radius: radius.clone(),
        });
    }

    fn adjust_corner<C: RoundedRectangleProgramCallbacks<Value = Value>>(
        &mut self,
        corner: usize,
        coordinate: usize,
        add: bool,
        radius: &Value,
        callbacks: &mut C,
    ) -> Result<(), RoundedRectangleProgramError<C::Error>> {
        if !self.corners[corner] {
            return Ok(());
        }
        let one = callbacks
            .integer(1)
            .map_err(RoundedRectangleProgramError::Callback)?;
        let offset = callbacks
            .add(radius, &one)
            .map_err(RoundedRectangleProgramError::Callback)?;
        let rectangle = self.adjusted_rectangle.as_mut().ok_or_else(|| {
            RoundedRectangleProgramError::Core(PilError::InternalError(
                "rounded rectangle adjustment has no rectangle".into(),
            ))
        })?;
        let current = &rectangle.bounds[coordinate];
        rectangle.bounds[coordinate] = if add {
            callbacks.in_place_add(current, &offset)
        } else {
            callbacks.in_place_subtract(current, &offset)
        }
        .map_err(RoundedRectangleProgramError::Callback)?;
        Ok(())
    }

    fn accept_response<Error>(
        &mut self,
        response: Option<RoundedRectangleProgramResponse>,
    ) -> Result<(), RoundedRectangleProgramError<Error>> {
        match (self.waiting.take(), response) {
            (
                Some(ProgramRequest::Corner(index)),
                Some(RoundedRectangleProgramResponse::Corner(value)),
            ) => {
                self.corners[index] = value;
            }
            (Some(ProgramRequest::Corner(_)), None) => {
                return Err(RoundedRectangleProgramError::Core(PilError::InternalError(
                    "rounded rectangle program requires a corner value".into(),
                )));
            }
            (
                Some(ProgramRequest::Width(WidthRequest::Arc(step))),
                Some(RoundedRectangleProgramResponse::Width),
            ) => {
                self.pending_draw = Some(step);
            }
            (
                Some(ProgramRequest::OutlineEnabled),
                Some(RoundedRectangleProgramResponse::OutlineEnabled(enabled)),
            ) => {
                if enabled {
                    if let Some(context) = self.outline.take() {
                        self.instructions.push_back(Instruction::Curves {
                            bounds: context.bounds.clone(),
                            diameter: context.diameter.clone(),
                            full_x: context.full_x,
                            full_y: context.full_y,
                            color: context.color,
                            outline: true,
                        });
                        self.append_outline_borders(context);
                    }
                } else {
                    self.outline = None;
                }
            }
            (Some(_), None) => {
                return Err(RoundedRectangleProgramError::Core(PilError::InternalError(
                    "rounded rectangle program requires a response".into(),
                )));
            }
            (None, Some(_)) => {
                return Err(RoundedRectangleProgramError::Core(PilError::InternalError(
                    "rounded rectangle program received an unexpected response".into(),
                )));
            }
            (Some(_), Some(_)) => {
                return Err(RoundedRectangleProgramError::Core(PilError::InternalError(
                    "rounded rectangle program received a mismatched response".into(),
                )));
            }
            (None, None) => {}
        }
        Ok(())
    }

    fn append_outline_borders(&mut self, context: OutlineContext<Value>) {
        if !context.full_x {
            self.instructions.push_back(Instruction::OutlineBorder(
                RoundedRectangleOutlineBorder::Top,
                context.clone(),
            ));
            self.instructions.push_back(Instruction::OutlineBorder(
                RoundedRectangleOutlineBorder::Bottom,
                context.clone(),
            ));
        }
        if !context.full_y {
            self.instructions.push_back(Instruction::OutlineBorder(
                RoundedRectangleOutlineBorder::Left,
                context.clone(),
            ));
            self.instructions.push_back(Instruction::OutlineBorder(
                RoundedRectangleOutlineBorder::Right,
                context,
            ));
        }
    }
}

fn push_curve_instruction<Value>(
    instructions: &mut VecDeque<Instruction<Value>>,
    bounds: RoundedRectangleBounds<Value>,
    diameter: Value,
    full_x: bool,
    full_y: bool,
    color: RoundedRectangleColor,
    outline: bool,
) {
    instructions.push_back(Instruction::Curves {
        bounds,
        diameter,
        full_x,
        full_y,
        color,
        outline,
    });
}

fn add_one<C: RoundedRectangleProgramCallbacks>(
    value: &C::Value,
    amount: &C::Value,
    callbacks: &mut C,
) -> Result<C::Value, C::Error> {
    let sum = callbacks.add(value, amount)?;
    let one = callbacks.integer(1)?;
    callbacks.add(&sum, &one)
}

fn subtract_one<C: RoundedRectangleProgramCallbacks>(
    value: &C::Value,
    amount: &C::Value,
    callbacks: &mut C,
) -> Result<C::Value, C::Error> {
    let difference = callbacks.subtract(value, amount)?;
    let one = callbacks.integer(1)?;
    callbacks.subtract(&difference, &one)
}

#[derive(Clone, Copy)]
enum CurveCorner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

fn curve_bounds<Value: Clone, C: RoundedRectangleProgramCallbacks<Value = Value>>(
    corner: CurveCorner,
    bounds: &RoundedRectangleBounds<Value>,
    diameter: &Value,
    callbacks: &mut C,
) -> Result<RoundedRectangleBounds<Value>, C::Error> {
    match corner {
        CurveCorner::TopLeft => {
            let right = callbacks.add(&bounds[0], diameter)?;
            let bottom = callbacks.add(&bounds[1], diameter)?;
            Ok([bounds[0].clone(), bounds[1].clone(), right, bottom])
        }
        CurveCorner::TopRight => {
            let left = callbacks.subtract(&bounds[2], diameter)?;
            let bottom = callbacks.add(&bounds[1], diameter)?;
            Ok([left, bounds[1].clone(), bounds[2].clone(), bottom])
        }
        CurveCorner::BottomRight => {
            let left = callbacks.subtract(&bounds[2], diameter)?;
            let top = callbacks.subtract(&bounds[3], diameter)?;
            Ok([left, top, bounds[2].clone(), bounds[3].clone()])
        }
        CurveCorner::BottomLeft => {
            let top = callbacks.subtract(&bounds[3], diameter)?;
            let right = callbacks.add(&bounds[0], diameter)?;
            Ok([bounds[0].clone(), top, right, bounds[3].clone()])
        }
    }
}

fn rounded_curve_parts<Value: Clone, C: RoundedRectangleProgramCallbacks<Value = Value>>(
    bounds: &RoundedRectangleBounds<Value>,
    diameter: &Value,
    full_x: bool,
    full_y: bool,
    callbacks: &mut C,
) -> Result<Vec<(RoundedRectangleBounds<Value>, f64, f64)>, C::Error> {
    if full_x {
        return Ok(vec![
            (
                curve_bounds(CurveCorner::TopLeft, bounds, diameter, callbacks)?,
                180.0,
                360.0,
            ),
            (
                curve_bounds(CurveCorner::BottomLeft, bounds, diameter, callbacks)?,
                0.0,
                180.0,
            ),
        ]);
    }
    if full_y {
        return Ok(vec![
            (
                curve_bounds(CurveCorner::TopLeft, bounds, diameter, callbacks)?,
                90.0,
                270.0,
            ),
            (
                curve_bounds(CurveCorner::TopRight, bounds, diameter, callbacks)?,
                270.0,
                90.0,
            ),
        ]);
    }
    Ok(vec![
        (
            curve_bounds(CurveCorner::TopLeft, bounds, diameter, callbacks)?,
            180.0,
            270.0,
        ),
        (
            curve_bounds(CurveCorner::TopRight, bounds, diameter, callbacks)?,
            270.0,
            360.0,
        ),
        (
            curve_bounds(CurveCorner::BottomRight, bounds, diameter, callbacks)?,
            0.0,
            90.0,
        ),
        (
            curve_bounds(CurveCorner::BottomLeft, bounds, diameter, callbacks)?,
            90.0,
            180.0,
        ),
    ])
}
