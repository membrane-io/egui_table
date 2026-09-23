use egui::{Rect, Ui, UiBuilder, Vec2, Vec2b, pos2, vec2};
/// A scroll area with some portion of its left and/or top side "stuck".
///
/// This produces four quadrants:
///
/// ```text
///               <-------LEFT-------> <---------RIGHT---------->
///
///              ------------------------------------------------
///          ^   |                    |   <----------------->   |
///    TOP   |   |       Fixed        |      Horizontally       |
///          V   |    fixed_size      |       scrollable        |
///              |--------------------|-------------------------|.................
///          ^   | ^                  |           ^             |                .
///  BOTTOM  |   | |   Vertically     | <-  Fully scrollable -> |                .
///          |   | |   scrollable     |    scroll_outer_size    |                .
///          V   | v                  |           v             |                .
///              |____________________|_________________________|                .
///                                   .                                          .
///                                   .                   scroll_content_size    .
///                                   .                                          .
///                                   ............................................
/// ```
///
/// The above shows the initial layout when the scroll offset is zero (no scrolling has occurred yet).
#[derive(Clone, Copy, Debug)]
pub struct SplitScroll {
  pub scroll_enabled: Vec2b,

  /// Width of the fixed left side, and height of the fixed top.
  pub fixed_size: Vec2,

  /// Size of the small container of the right bottom scrollable region.
  pub scroll_outer_size: Vec2,

  /// Size of the large contents of the right bottom region, ignoring the left/top fixed regions.
  pub scroll_content_size: Vec2,

  /// If true, the vertical scrollbar will stick to the bottom as the content grows.
  pub stick_to_bottom: bool,

  /// Salt for the inner [`egui::ScrollArea`]. Must be unique among tables in the same parent `Ui`.
  pub id_salt: egui::Id,
}

/// The contents of a [`SplitScroll`].
pub trait SplitScrollDelegate {
  /// The fixed portion of the top left corner.
  fn left_top_ui(&mut self, ui: &mut Ui);

  /// The horizontally scrollable portion.
  fn right_top_ui(&mut self, ui: &mut Ui);

  /// The vertically scrollable portion.
  fn left_bottom_ui(&mut self, ui: &mut Ui);

  /// The fully scrollable portion.
  ///
  /// First to be called.
  fn right_bottom_ui(&mut self, ui: &mut Ui);

  /// Called last.
  fn finish(&mut self, _ui: &mut Ui) {}

  /// The scroll offset the body used for this frame. Positive x moves content left.
  fn set_scroll_offset(&mut self, _offset: Vec2) {}
}

/// Geometry of one [`SplitScroll`] frame.
pub struct SplitScrollOutput {
  /// Scroll offset applied to the body. Positive x hides content on the left.
  pub offset: Vec2,

  /// Size of the scrolling content, excluding the fixed left and top.
  pub content_size: Vec2,

  /// Visible rectangle of the scrolling region.
  pub viewport: Rect,

  /// Full rectangle, including the fixed left and top.
  pub rect: Rect,
}

/// A grid-aligned rect inside `bounds`.
///
/// [`Ui::advance_cursor_after_rect`] rounds each edge onto the ui grid. That rounded
/// rect can extend past `bounds`. The caller then stores the extra height.
fn rect_inside_grid(desired: Rect, bounds: Rect) -> Rect {
  use egui::emath::GuiRounding as _;

  let grid = egui::emath::GUI_ROUNDING;
  let snap_min = |value: f32, limit: f32| -> f32 {
    let floored = value.max(limit).floor_ui();
    if floored < limit { floored + grid } else { floored }
  };
  let snap_max = |value: f32, limit: f32| value.min(limit).floor_ui();
  let min_x = snap_min(desired.min.x, bounds.min.x);
  let min_y = snap_min(desired.min.y, bounds.min.y);
  Rect::from_min_max(
    pos2(min_x, min_y),
    pos2(snap_max(desired.max.x, bounds.max.x).max(min_x), snap_max(desired.max.y, bounds.max.y).max(min_y)),
  )
}

impl SplitScroll {
  pub fn show(self, ui: &mut Ui, delegate: &mut dyn SplitScrollDelegate) -> SplitScrollOutput {
    let Self { scroll_enabled, fixed_size, scroll_outer_size, scroll_content_size, stick_to_bottom, id_salt } = self;

    let bounds = ui.available_rect_before_wrap().intersect(ui.max_rect());
    ui.scope_builder(UiBuilder::new().max_rect(bounds), |ui| {
      ui.visuals_mut().clip_rect_margin = 0.0; // Everything else looks awful

      let limit = ui.max_rect();
      let desired_max = limit.min + fixed_size + scroll_outer_size;
      let desired = Rect::from_min_max(
        limit.min,
        pos2(desired_max.x.min(limit.max.x).max(limit.min.x), desired_max.y.min(limit.max.y).max(limit.min.y)),
      );
      // `advance_cursor_after_rect` rounds again. A rect that is already on the grid stays put.
      let rect = rect_inside_grid(desired, limit);
      ui.shrink_clip_rect(rect);
      let rect = rect;

      let bottom_right_rect = Rect::from_min_max(rect.min + fixed_size, rect.max);

      let scroll_offset = {
        // RIGHT BOTTOM: fully scrollable.

        // The entire thing is a `ScrollArea` that we then paint over.
        // PROBLEM: scroll bars show up at the full rect, instead of just the bottom-right.
        // We could add something like `ScrollArea::with_scroll_bar_rect(bottom_right_rect)`

        let mut scroll_ui = ui.new_child(UiBuilder::new().max_rect(rect));

        egui::ScrollArea::new(scroll_enabled)
          .id_salt(id_salt)
          .auto_shrink(false)
          .scroll_bar_rect(bottom_right_rect)
          .stick_to_bottom(stick_to_bottom)
          .show_viewport(&mut scroll_ui, |ui, scroll_offset| {
            ui.set_min_size(fixed_size + scroll_content_size);

            let mut shrunk_rect = ui.max_rect();
            shrunk_rect.min += fixed_size;

            let mut shrunk_ui = ui.new_child(UiBuilder::new().max_rect(shrunk_rect));
            shrunk_ui.shrink_clip_rect(bottom_right_rect);
            // ScrollArea places this content at viewport.min, then rounds the content rect to pixels.
            // An offset taken from that rect differs from the header strip by up to one pixel.
            delegate.set_scroll_offset(scroll_offset.min.to_vec2());
            delegate.right_bottom_ui(&mut shrunk_ui);

            // It is very important that the scroll offset is synced between the
            // right-bottom contents of the real scroll area,
            // and the fake scroll areas we are painting later.
            // The scroll offset that `ScrollArea` returns could be a newer one
            // than was used for rendering, so we use the one _actually_ used for rendering instead:
            scroll_offset.min
          })
          .inner
      };

      {
        // LEFT TOP: Fixed
        let left_top_rect = rect.with_max_x(rect.left() + fixed_size.x).with_max_y(rect.top() + fixed_size.y);
        let mut left_top_ui = ui.new_child(UiBuilder::new().max_rect(left_top_rect));
        left_top_ui.shrink_clip_rect(left_top_rect);
        delegate.left_top_ui(&mut left_top_ui);
      }

      {
        // RIGHT TOP: Horizontally scrollable
        let right_top_outer_rect = rect.with_min_x(rect.left() + fixed_size.x).with_max_y(rect.top() + fixed_size.y);
        let right_top_content_rect = Rect::from_min_size(
          pos2(right_top_outer_rect.min.x - scroll_offset.x, rect.min.y),
          vec2(scroll_content_size.x, fixed_size.y),
        );
        let mut right_top_ui = ui.new_child(UiBuilder::new().max_rect(right_top_content_rect));
        right_top_ui.shrink_clip_rect(right_top_outer_rect);
        delegate.right_top_ui(&mut right_top_ui);
      }

      {
        // LEFT BOTTOM: Vertically scrollable
        let left_bottom_outer_rect = rect.with_max_x(rect.left() + fixed_size.x).with_min_y(rect.top() + fixed_size.y);
        let left_bottom_content_rect = Rect::from_min_size(
          pos2(rect.min.x, left_bottom_outer_rect.min.y - scroll_offset.y),
          vec2(fixed_size.x, scroll_content_size.y),
        );
        let mut left_bottom_ui = ui.new_child(UiBuilder::new().max_rect(left_bottom_content_rect));
        left_bottom_ui.shrink_clip_rect(left_bottom_outer_rect);
        delegate.left_bottom_ui(&mut left_bottom_ui);
      }

      delegate.finish(ui);
      ui.advance_cursor_after_rect(rect);

      SplitScrollOutput {
        offset: scroll_offset.to_vec2(),
        content_size: scroll_content_size,
        viewport: bottom_right_rect,
        rect,
      }
    })
    .inner
  }
}
