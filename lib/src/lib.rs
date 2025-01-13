use std::collections::HashMap;

use itertools::Itertools;
use metamath_knife::{
    database::DbOptions,
    scopeck::{Frame, Hyp},
    statement::{StatementAddress, Token, TokenPtr},
    Database, StatementRef, StatementType,
};

pub struct Context {
    metamath_db: Database,
}

fn from_utf8(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes).unwrap().into()
}

impl Context {
    pub fn load(name: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        let name = name.into();
        let data = data.into();
        let mut metamath_db = Database::new(DbOptions {
            autosplit: true,
            ..Default::default()
        });
        metamath_db.parse(name.clone(), vec![(name, data)]);
        metamath_db.scope_pass();
        metamath_db.typesetting_pass();
        Self { metamath_db }
    }

    pub fn initial_state(&self, level: Option<&str>) -> State {
        let db = &self.metamath_db;
        let current_level_stmt_addr = match level {
            Some(level) => db.statement(level.as_bytes()).unwrap().address(),
            None => db
                .statements()
                .find(|stmt| stmt.statement_type() == StatementType::Provable)
                .expect(
                    format!("only {} statements", self.metamath_db.statements().count()).as_str(),
                )
                .address(),
        };
        State {
            current_level_stmt_addr,
            proof_stack: Vec::new(),
        }
    }

    fn deps(&self, addr: StatementAddress) -> Vec<StatementAddress> {
        let db = &self.metamath_db;
        let stmt = db.statement_by_address(addr);
        let proof_tree = db.get_proof_tree(stmt).unwrap();
        proof_tree
            .with_steps(db, |_cur, stmt, _hyps| stmt.address())
            .into_iter()
            .unique()
            .collect()
    }

    fn render_token(&self, token: TokenPtr) -> String {
        let typesetting_data = self.metamath_db.typesetting_result();
        match typesetting_data.get_alt_html_def(&token) {
            Some(token) => from_utf8(token),
            None => from_utf8(&token),
        }
    }

    fn render_expr(&self, expr: &[Token]) -> String {
        expr.into_iter().map(|tok| self.render_token(tok)).collect()
    }

    fn render_stmt(&self, stmt: StatementAddress) -> String {
        self.metamath_db
            .statement_by_address(stmt)
            .math_iter()
            .map(|tok| self.render_token(tok.slice))
            .collect()
    }

    fn render_ascii_stmt(&self, stmt: StatementAddress) -> String {
        self.metamath_db
            .statement_by_address(stmt)
            .math_iter()
            .map(|tok| from_utf8(&tok))
            .collect::<Vec<String>>()
            .join(" ")
    }

    fn hyp_addrs(&self, stmt_addr: StatementAddress) -> Vec<StatementAddress> {
        let db = &self.metamath_db;
        let stmt = db.statement_by_address(stmt_addr);
        let scopes = db.scope_result();
        match scopes.get(stmt.label()) {
            Some(frame) => frame
                .hypotheses
                .iter()
                .map(|hyp| match hyp {
                    Hyp::Floating(hyp_addr, _, _) | Hyp::Essential(hyp_addr, _) => *hyp_addr,
                })
                .collect::<Vec<StatementAddress>>(),
            None => Vec::new(),
        }
    }

    pub fn render_inference(&self, stmt_addr: StatementAddress) -> String {
        let conclusion = self.render_stmt(stmt_addr);
        let hyps = self
            .hyp_addrs(stmt_addr)
            .into_iter()
            .map(|hyp_addr| format!("{} <br/>", self.render_stmt(hyp_addr)))
            .collect::<String>();
        let inner = if hyps.is_empty() {
            conclusion
        } else {
            format!("{hyps} <hr/> {conclusion}")
        };
        format!("<div style='display: inline-block'> {inner} </div>")
    }
    pub fn render_ascii_inference(&self, stmt_addr: StatementAddress) -> String {
        let conclusion = self.render_ascii_stmt(stmt_addr);
        let hyps = self
            .hyp_addrs(stmt_addr)
            .into_iter()
            .map(|hyp_addr| self.render_ascii_stmt(hyp_addr))
            .collect::<Vec<String>>()
            .join("   &   ");
        if hyps.is_empty() {
            conclusion
        } else {
            format!("{hyps}   ->   {conclusion}")
        }
    }

    pub fn label(&self, stmt: StatementAddress) -> String {
        let l = self.metamath_db.statement_by_address(stmt).label();
        from_utf8(l)
    }
}

/// The state game
#[derive(Clone)]
pub struct State {
    /// Index of the statement representing the current level
    pub current_level_stmt_addr: StatementAddress,

    pub proof_stack: Vec<Vec<Token>>,
}

fn stmt_to_expr(stmt: StatementRef) -> Vec<Token> {
    stmt.math_iter().map(|tok| tok.slice.into()).collect()
}

impl State {
    fn push(&self, ctx: &Context, step_addr: StatementAddress) -> Option<Self> {
        let db = &ctx.metamath_db;
        let step_stmt = db.statement_by_address(step_addr);
        let step_type = step_stmt.statement_type();
        let proof_stack = if let StatementType::Essential | StatementType::Floating = step_type {
            let expr = stmt_to_expr(step_stmt);
            let mut stack = self.proof_stack.clone();
            stack.push(expr);
            stack
        } else {
            if !matches!(step_type, StatementType::Axiom | StatementType::Provable) {
                return None;
            }
            let names = db.name_result();
            let scopes = db.scope_result();
            let step_frame = scopes.get(step_stmt.label())?;
            let conclusion = stmt_to_expr(step_stmt);
            let Frame {
                mandatory_dv: dvs,
                hypotheses: hyps,
                ..
            } = step_frame;
            let npop = hyps.len();
            let mut stack = self.proof_stack.clone();
            let mut sp = stack.len().checked_sub(npop)?;
            let mut subst = HashMap::<Token, Vec<Token>>::new();
            for hyp in hyps.iter() {
                match hyp {
                    Hyp::Floating(_, var_index, typecode) => {
                        let entry = stack.get(sp)?;
                        if &**entry.first()? != names.atom_name(*typecode) {
                            return None;
                        }
                        let var_atom = *step_frame.var_list.get(*var_index)?;
                        let var = names.atom_name(var_atom);
                        subst.insert(var.into(), entry.get(1..)?.to_vec());
                        sp += 1;
                    }
                    Hyp::Essential(h_addr, _) => {
                        let h_stmt = db.statement_by_address(*h_addr);
                        let h = stmt_to_expr(h_stmt);
                        let entry = stack.get(sp)?;
                        let subst_h = h
                            .into_iter()
                            .map(|tok| subst.get(&tok).unwrap_or(&vec![tok]).clone())
                            .flatten()
                            .collect::<Vec<Token>>();
                        if entry != &subst_h {
                            return None;
                        }
                        sp += 1;
                    }
                }
                for (x_index, y_index) in dvs.iter() {
                    let x_atom = *step_frame.var_list.get(*x_index)?;
                    let y_atom = *step_frame.var_list.get(*y_index)?;
                    let x = names.atom_name(x_atom);
                    let y = names.atom_name(y_atom);
                    let x_vars = subst
                        .get(x)?
                        .iter()
                        .filter(|tok| step_frame.var_list.contains(&names.get_atom(tok)))
                        .cloned()
                        .collect::<Vec<Token>>();
                    let y_vars = subst
                        .get(y)?
                        .iter()
                        .filter(|tok| step_frame.var_list.contains(&names.get_atom(tok)))
                        .cloned()
                        .collect::<Vec<Token>>();
                    for (x0, y0) in x_vars.into_iter().cartesian_product(y_vars) {
                        if x0 == y0 {
                            return None;
                        }
                        let level_stmt = db.statement_by_address(self.current_level_stmt_addr);
                        let level_frame = scopes.get(level_stmt.label())?;
                        let level_dvs = level_frame
                            .mandatory_dv
                            .iter()
                            .map(|(x, y)| {
                                Some((
                                    names.atom_name(*level_frame.var_list.get(*x)?).into(),
                                    names.atom_name(*level_frame.var_list.get(*y)?).into(),
                                ))
                            })
                            .collect::<Option<Vec<_>>>()?;
                        if !(level_dvs.contains(&(x0.clone(), y0.clone()))
                            || level_dvs.contains(&(y0, x0)))
                        {
                            return None;
                        }
                    }
                }
            }
            for _ in 0..npop {
                stack.pop()?;
            }
            let subst_conclusion = conclusion
                .iter()
                .map(|tok| subst.get(tok).unwrap_or(&vec![tok.clone()]).clone())
                .flatten()
                .collect::<Vec<Token>>();
            stack.push(subst_conclusion);
            stack
        };
        Some(Self {
            proof_stack,
            ..*self
        })
    }

    fn level_finished(&self, ctx: &Context) -> bool {
        let db = &ctx.metamath_db;
        let level_stmt = db.statement_by_address(self.current_level_stmt_addr);
        let level_goal = stmt_to_expr(level_stmt);
        self.proof_stack.last() == Some(&level_goal)
    }

    pub fn next_level(&self, ctx: &Context) -> Option<Self> {
        if self.level_finished(ctx) {
            Some(State {
                current_level_stmt_addr: ctx
                    .metamath_db
                    .statements_range_address(self.current_level_stmt_addr..)
                    .skip(1)
                    .find(|stmt| stmt.statement_type() == StatementType::Provable)
                    .expect(
                        format!("only {} statements", ctx.metamath_db.statements().count())
                            .as_str(),
                    )
                    .address(),

                proof_stack: Vec::new(),
            })
        } else {
            None
        }
    }

    pub fn buttons(&self, ctx: &Context) -> Vec<(StatementAddress, Option<Self>)> {
        ctx.deps(self.current_level_stmt_addr)
            .into_iter()
            .map(|addr| (addr, self.push(ctx, addr)))
            .collect()
    }

    pub fn render_stack(&self, ctx: &Context) -> Vec<String> {
        self.proof_stack
            .iter()
            .map(|expr| ctx.render_expr(expr))
            .collect()
    }

    pub fn stack_swap(&self, i: usize, j: usize) -> Self {
        let mut new = self.clone();
        new.proof_stack.swap(i, j);
        new
    }
}
