use std::{collections::HashSet, iter};

use itertools::Itertools;
use metamath_rs::{
    database::DbOptions,
    formula::Substitutions,
    nameck::{Atom, NameReader},
    scopeck::Hyp,
    statement::TokenPtr,
    Database, Formula, StatementRef, StatementType,
};

pub use metamath_rs::statement::StatementAddress;

pub struct Context {
    metamath_db: Database,
}

fn from_utf8(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes).unwrap().into()
}

fn formula_eq(a: &Formula, b: &Formula) -> bool {
    a == b && a.get_typecode() == b.get_typecode()
}

impl Context {
    pub fn load(name: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        let name = name.into();
        let data = data.into();
        let mut metamath_db = Database::new(DbOptions {
            autosplit: true,
            incremental: true,
            ..Default::default()
        });
        metamath_db.parse(name.clone(), vec![(name, data)]);
        metamath_db.scope_pass();
        metamath_db.typesetting_pass();
        metamath_db.grammar_pass();
        Self { metamath_db }
    }

    pub fn initial_state(&self, level: Option<&str>) -> State {
        let db = &self.metamath_db;
        let current_level_stmt_addr = match level {
            Some(level) => db.statement(level.as_bytes()).unwrap().address(),
            None => db
                .statements()
                .find(|stmt| stmt.statement_type() == StatementType::Provable)
                .unwrap_or_else(|| {
                    panic!("only {} statements", self.metamath_db.statements().count())
                })
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
        match typesetting_data.latex_defs.get(token) {
            Some((_, _, token)) => from_utf8(token),
            None => from_utf8(token),
        }
    }

    fn render_formula(&self, formula: &Formula) -> String {
        let db = &self.metamath_db;
        let names = db.name_result();
        iter::once(formula.get_typecode())
            .chain(formula.as_ref(db))
            .map(|tok| self.render_token(names.atom_name(tok)))
            .collect()
    }

    fn render_stmt(&self, stmt: StatementAddress) -> String {
        self.metamath_db
            .statement_by_address(stmt)
            .math_iter()
            .map(|tok| self.render_token(tok.slice))
            .collect()
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

    pub fn render_inference(&self, stmt_addr: StatementAddress) -> (Vec<String>, String) {
        let conclusion = self.render_stmt(stmt_addr);
        let hyps = self
            .hyp_addrs(stmt_addr)
            .into_iter()
            .map(|hyp_addr| self.render_stmt(hyp_addr))
            .collect();
        (hyps, conclusion)
    }

    pub fn label(&self, stmt: StatementAddress) -> String {
        let l = self.metamath_db.statement_by_address(stmt).label();
        from_utf8(l)
    }

    fn stmt_to_formula(&self, stmt: StatementRef) -> Formula {
        let db = &self.metamath_db;
        let grammar = db.grammar_result();
        let names = db.name_result();
        grammar
            .parse_statement(&stmt, names, &mut NameReader::new(names))
            .unwrap()
    }

    fn unify_hyps(&self, hyps: &[&Hyp], stack_top: &[Formula]) -> Option<Substitutions> {
        let db = &self.metamath_db;

        // Ensure no essential hypotheses are ignored
        if hyps
            .iter()
            .rev()
            .skip(stack_top.len())
            .any(|hyp| matches!(hyp, Hyp::Essential(_, _)))
        {
            return None;
        }

        let mut substs = Substitutions::new();
        for (stack_hyp, hyp) in stack_top.iter().rev().zip(hyps.iter().rev()) {
            let hyp = db.statement_by_address(hyp.address());
            let hyp = self.stmt_to_formula(hyp);
            if stack_hyp.get_typecode() != hyp.get_typecode() {
                return None;
            }
            stack_hyp.unify(&hyp, &mut substs).ok()?
        }

        Some(substs)
    }
}

/// The game state
#[derive(Clone, Debug)]
pub struct State {
    /// Index of the statement representing the current level
    pub current_level_stmt_addr: StatementAddress,

    pub proof_stack: Vec<Formula>,
}

impl State {
    fn push(&self, ctx: &Context, step_addr: StatementAddress) -> Option<Self> {
        let db = &ctx.metamath_db;
        let names = db.name_result();
        let scopes = db.scope_result();
        let step_stmt = db.statement_by_address(step_addr);
        let step_type = step_stmt.statement_type();
        let proof_stack = if let StatementType::Essential | StatementType::Floating = step_type {
            let formula = ctx.stmt_to_formula(step_stmt);
            let mut stack = self.proof_stack.clone();
            stack.push(formula);
            stack
        } else {
            if !matches!(step_type, StatementType::Axiom | StatementType::Provable) {
                return None;
            }
            let step_frame = scopes.get(step_stmt.label())?;
            let conclusion = ctx.stmt_to_formula(step_stmt);
            let hyps = &step_frame.hypotheses;
            let mut stack = self.proof_stack.clone();
            let max_num_pop = stack.len().min(hyps.len());
            let (substs, num_pop) = hyps
                .iter()
                .permutations(hyps.len())
                .cartesian_product(1..=max_num_pop)
                .find_map(|(hyps, num_pop)| {
                    let sp = stack.len().checked_sub(num_pop)?;
                    let stack_hyps = &stack[sp..];
                    let substs = ctx.unify_hyps(&hyps, stack_hyps)?;
                    let subst_vars = substs
                        .iter()
                        .map(|(var, _)| {
                            names.get_atom(&db.statement_by_label(*var).unwrap().math_at(1))
                        })
                        .collect::<HashSet<Atom>>();
                    let step_vars = step_frame
                        .var_list
                        .iter()
                        .copied()
                        .collect::<HashSet<Atom>>();
                    if subst_vars == step_vars {
                        Some((substs, num_pop))
                    } else {
                        None
                    }
                })?;
            for _ in 0..num_pop {
                stack.pop()?;
            }
            let subst_conclusion = conclusion.substitute(&substs);
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
        let level_goal = ctx.stmt_to_formula(level_stmt);
        if let Some(stack_last) = self.proof_stack.last() {
            formula_eq(stack_last, &level_goal)
        } else {
            false
        }
    }

    pub fn next_level(&self, ctx: &Context) -> Option<Self> {
        if self.level_finished(ctx) {
            Some(State {
                current_level_stmt_addr: ctx
                    .metamath_db
                    .statements_range_address(self.current_level_stmt_addr..)
                    .skip(1)
                    .find(|stmt| stmt.statement_type() == StatementType::Provable)
                    .unwrap_or_else(|| {
                        panic!("only {} statements", ctx.metamath_db.statements().count())
                    })
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
            .map(|expr| ctx.render_formula(expr))
            .collect()
    }

    pub fn stack_swap(&self, i: usize, j: usize) -> Self {
        let mut new = self.clone();
        new.proof_stack.swap(i, j);
        new
    }

    pub fn stack_delete(&self, i: usize) -> Self {
        let mut new = self.clone();
        new.proof_stack.remove(i);
        new
    }

    pub fn stack_copy(&self, i: usize) -> Self {
        let mut new = self.clone();
        new.proof_stack.insert(i, new.proof_stack[i].clone());
        new
    }

    pub fn stack_move(&self, src: usize, dst: usize) -> Self {
        let mut new = self.clone();
        let e = new.proof_stack.remove(src);
        new.proof_stack.insert(dst, e);
        new
    }
}
