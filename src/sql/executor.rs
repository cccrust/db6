//! SQL executor — query execution (移植自 sql6/src/planner/executor.rs).
//!
//! 需修改：將呼叫 pager 的部分改為呼叫 StorageEngine trait。

use crate::engine::StorageEngine;
use crate::error::Result;

pub struct Executor {
    engine: Box<dyn StorageEngine>,
}

impl Executor {
    pub fn new(engine: Box<dyn StorageEngine>) -> Self {
        Executor { engine }
    }

    pub fn execute(&mut self, sql: &str) -> Result<ResultSet> {
        let stmts = crate::sql::parser::parse(sql)?;
        let mut results = Vec::new();
        for stmt in stmts {
            let plan = self.plan(&stmt)?;
            let rs = self.execute_plan(&plan)?;
            results.push(rs);
        }
        Ok(ResultSet { rows: results })
    }

    fn plan(&self, stmt: &crate::sql::parser::ast::Statement) -> Result<crate::sql::planner::Plan> {
        todo!("移植並改為呼叫 StorageEngine")
    }

    fn execute_plan(&mut self, plan: &crate::sql::planner::Plan) -> Result<RowResult> {
        todo!("移植並改為呼叫 StorageEngine.get/scan/put/delete")
    }
}

pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<RowResult>,
    pub affected: usize,
}

pub struct RowResult;