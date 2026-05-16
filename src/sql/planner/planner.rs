//! Query planner（移植自 sql6/src/planner/planner.rs）
//!
//! 需修改：根據 engine type 限制不支援的 SQL 語法（LSM 不支援 JOIN、複雜 ORDER BY）

use crate::engine::StorageEngine;
use crate::error::{Error, Result};
use crate::sql::parser::ast::*;
use crate::sql::planner::plan::*;

pub struct Planner;

impl Planner {
    pub fn new() -> Self {
        Planner
    }

    pub fn plan(&self, stmt: &Statement, engine_type: &str) -> Result<Plan> {
        match stmt {
            Statement::Select(s) => self.plan_select(s, engine_type),
            Statement::Insert(i) => Ok(Plan::Insert(InsertPlan {
                table: i.table.clone(),
                values: i.values.clone(),
            })),
            Statement::Update(u) => Ok(Plan::Update(UpdatePlan {
                table: u.table.clone(),
                set: u.set.clone(),
                filter: u.where_clause.as_ref().map(|e| (*e.clone()).clone()),
            })),
            Statement::Delete(d) => Ok(Plan::Delete(DeletePlan {
                table: d.table.clone(),
                filter: d.where_clause.as_ref().map(|e| (*e.clone()).clone()),
            })),
            Statement::CreateTable(c) => Ok(Plan::CreateTable(CreateTablePlan {
                name: c.name.clone(),
                columns: c.columns.clone(),
                if_not_exists: c.if_not_exists,
            })),
            Statement::CreateVirtualTable(c) => {
                if engine_type == "lsm" {
                    return Err(Error::NotSupported("FTS not supported with LSM engine".into()));
                }
                Ok(Plan::CreateFtsTable(FtsPlan {
                    name: c.name.clone(),
                    columns: c.columns.clone(),
                    tokenize: c.tokenize.clone(),
                }))
            }
            Statement::DropTable(d) => Ok(Plan::DropTable(DropTablePlan {
                name: d.name.clone(),
                if_exists: d.if_exists,
            })),
        }
    }

    fn plan_select(&self, s: &SelectStmt, engine_type: &str) -> Result<Plan> {
        let table = s.from.clone().unwrap_or_default();
        let (is_fts, fts_query) = self.detect_fts(&s.where_clause);

        if is_fts && engine_type == "lsm" {
            return Err(Error::NotSupported("FTS not supported with LSM engine".into()));
        }

        if let Some(ref order_by) = s.order_by {
            if engine_type == "lsm" && !order_by.is_empty() {
                return Err(Error::NotSupported(
                    "ORDER BY not supported with LSM engine (high I/O cost)".into(),
                ));
            }
        }

        Ok(Plan::Scan(ScanPlan {
            table,
            filter: s.where_clause.as_ref().map(|e| (*e.clone()).clone()),
            order_by: s.order_by.clone().unwrap_or_default(),
            limit: s.limit,
            is_fts,
            fts_query,
        }))
    }

    fn detect_fts(&self, expr: &Option<Box<Expr>>) -> (bool, Option<String>) {
        match expr {
            Some(e) => self.find_fts_match(e),
            None => (false, None),
        }
    }

    fn find_fts_match(&self, expr: &Expr) -> (bool, Option<String>) {
        match expr {
            Expr::FtsMatch(fm) => (true, Some(fm.query.clone())),
            Expr::BinaryOp(left, _, right) => {
                let (l_fts, l_q) = self.find_fts_match(left);
                if l_fts { return (true, l_q); }
                self.find_fts_match(right)
            }
            _ => (false, None),
        }
    }
}

impl Default for Planner {
    fn default() -> Self {
        Self::new()
    }
}