//! Table viewer for [egui](https://www.egui.rs/).
//!
//! See [`Table`].

pub mod columns;
mod split_scroll;
mod table;

pub use columns::Column;
pub use split_scroll::{SplitScroll, SplitScrollDelegate, SplitScrollOutput};
pub use table::{
  AutoSizeMode, CellInfo, HeaderCellInfo, HeaderRow, PrefetchInfo, Table, TableDelegate, TableOutput, TableState,
};
