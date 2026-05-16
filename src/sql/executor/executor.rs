//! Query executor（移植自 sql6/src/planner/executor.rs）
//!
//! 需修改：將呼叫 pager 的部分改為呼叫 StorageEngine trait。

use crate::engine::StorageEngine;
use crate::error::{Error, Result};
use crate::sql::planner::{Plan, Planner};
use crate::sql::parser::parse;

pub struct Executor {
    engine: Box<dyn StorageEngine>,
    planner: Planner,
}

impl Executor {
    pub fn new(engine: Box<dyn StorageEngine>) -> Self {
        Executor {
            engine,
            planner: Planner::new(),
        }
    }

    pub fn execute(&mut self, sql: &str) -> Result<ResultSet> {
        let stmts = parse(sql).map_err(|e| Error::Sql(e.to_string()))?;
        let engine_type = self.engine.engine_type();

        let mut all_rows = Vec::new();
        for stmt in stmts {
            let plan = self.planner.plan(&stmt, engine_type)?;
            let rs = self.execute_plan(&plan)?;
            all_rows.extend(rs.rows);
        }
        Ok(ResultSet {
            columns: vec![],
            rows: all_rows,
            affected: 0,
        })
    }

    fn execute_plan(&mut self, plan: &Plan) -> Result<ResultSet> {
        match plan {
            Plan::Scan(s) => self.execute_scan(s),
            Plan::Insert(_) => todo!("implement INSERT"),
            Plan::Update(_) => todo!("implement UPDATE"),
            Plan::Delete(_) => todo!("implement DELETE"),
            Plan::CreateTable(_) => todo!("implement CREATE TABLE"),
            Plan::CreateFtsTable(f) => self.execute_create_fts(f),
            Plan::DropTable(_) => todo!("implement DROP TABLE"),
            Plan::Empty => Ok(ResultSet::default()),
        }
    }

    fn execute_scan(&self, s: &crate::sql::planner::ScanPlan) -> Result<ResultSet> {
        if s.is_fts {
            return Err(Error::Fts("FTS scan not yet implemented — see fts/ module".into()));
        }
        let start = b"".to_vec();
        let end = b"".to_vec();
        let rows = self.engine.scan(1, &start, &end)?;
        Ok(ResultSet {
            columns: vec![],
            rows: vec![],
            affected: 0,
        })
    }

    fn execute_create_fts(&mut self, f: &crate::sql::planner::FtsPlan) -> Result<ResultSet> {
        Ok(ResultSet {
            columns: vec![],
            rows: vec![],
            affected: 0,
        })
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new(Box::new(crate::engine::MemoryEngine::open_memory()))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<RowResult>,
    pub affected: usize,
}

#[derive(Debug, Clone)]
pub struct RowResult;