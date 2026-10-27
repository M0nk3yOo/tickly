//! Tickly Core – die gemeinsame Berechnungslogik für Backend und Companion-App.
//!
//! Enthält Soll/Ist-Rechnung, Brutto/Netto mit automatischen Pausen, Feiertage je
//! Bundesland, Abwesenheiten sowie Tages-/Wochen-/Monats- und Gesamtsalden.
//! Keine I/O, keine Uhr: Zeit wird immer als Parameter übergeben (testbar).

pub mod calc;
pub mod day;
pub mod demo;
pub mod holidays;
pub mod model;

pub use calc::*;
pub use day::*;
pub use model::*;

#[cfg(test)]
mod tests;
