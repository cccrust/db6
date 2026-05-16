//! SQL planner — plan generation (移植自 sql6/src/planner/).

use crate::error::Result;

pub mod plan;
pub mod planner;
pub mod constraints;

pub struct Planner;

impl Planner {
    pub fn new() -> Self {
        Planner
    }

    pub fn plan(&self, stmt: &crate::sql::parser::ast::Statement) -> Result<plan::Plan> {
        todo!("移植自 sql6/src/planner/planner.rs")
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}