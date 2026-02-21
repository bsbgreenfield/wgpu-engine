use std::{iter::Peekable, slice::Iter};

use crate::app::render::{Instruction, Operations, VMValue, renderer::Renderer};

struct InstructionSet<'frame> {
    instructions: Peekable<Iter<'frame, Instruction>>,
    pointer: usize,
}

impl<'frame> Renderer<'frame> {
    pub(super) fn interpret(&mut self, constants: Vec<VMValue>, instructions: Vec<Instruction>) {
        let mut stack: Vec<VMValue> = Vec::with_capacity(256);
        let mut pointer = 0;
        let mut instr_peek = instructions.iter().peekable();

        while instr_peek.peek().is_some() {}
    }

    fn doo_wop(instructions: &[Instruction], stack: &mut Vec<VMValue>, op: Operations) {
        match op {
            Operations::AddEntity => {}
            Operations::MoveEntity => {
                todo!()
            }
        }
    }
}
