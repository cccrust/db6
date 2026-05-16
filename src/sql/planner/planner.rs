//! Query planner（移植自 sql6/src/planner/planner.rs）

use crate::engine::StorageEngine;
use crate::error::{Error, Result};
use crate::sql::parser::ast::*;
use crate::sql::planner::plan::*;

pub struct Planner;

impl Planner {
    pub fn new() -> Self {
        Planner
    }

    pub fn plan(&self, stmt: &Statement, _engine_type: &str) -> Result<Plan> {
        match stmt {
            Statement::Select(s) => self.plan_select(s),
            Statement::Insert(i) => Ok(Plan::Insert(InsertPlan {
                table: i.table.clone(),
                values: i.values.clone(),
            })),
            Statement::Update(u) => Ok(Plan::Update(UpdatePlan {
                table: u.table.clone(),
                set: u.sets.clone(),
                filter: u.where_.clone(),
            })),
            Statement::Delete(d) => Ok(Plan::Delete(DeletePlan {
                table: d.table.clone(),
                filter: d.where_.clone(),
            })),
            Statement::CreateTable(c) => Ok(Plan::CreateTable(CreateTablePlan {
                name: c.name.clone(),
                columns: c.columns.clone(),
                if_not_exists: c.if_not_exists,
            })),
            Statement::DropTable(d) => Ok(Plan::DropTable(DropTablePlan {
                name: d.name.clone(),
                if_exists: d.if_exists,
            })),
            _ => Ok(Plan::Empty),
        }
    }

    fn plan_select(&self, s: &SelectStmt) -> Result<Plan> {
        let table = match &s.from {
            Some(FromItem::Table(t)) => t.name.clone(),
            _ => String::new(),
        };

        Ok(Plan::Scan(ScanPlan {
            table,
            filter: s.where_.clone(),
            order_by: s.order_by.clone(),
            limit: None,
            is_fts: false,
            fts_query: None,
        }))
    }
}