use std::{iter::Peekable, slice::Iter};

use crate::app::render::{
    Instruction, Operations, VMValue,
    renderer::{RenderUpdateDelta, Renderer},
};

type InstructionSet<'a> = Peekable<Iter<'a, Instruction>>;
impl<'frame> Renderer<'frame> {
    unsafe fn get_asset_ref(instr_peek: &mut Peekable<Iter<'_, Instruction>>) {
        let a: &Instruction = instr_peek.next().unwrap().try_into().unwrap();
    }

    fn get_constant_idx(instructions: &mut InstructionSet) -> u8 {
        let instr = instructions.next().expect("should define a constant idx");
        match instr {
            Instruction::ConstIdx(idx) => *idx,
            _ => panic!("expected a constant idx"),
        }
    }
    pub(super) fn interpret(
        &mut self,
        constants: Vec<VMValue>,
        instructions: Vec<Instruction>,
        queue: &wgpu::Queue,
    ) -> Vec<RenderUpdateDelta> {
        let mut res: Vec<RenderUpdateDelta> = Vec::new();
        let mut instr_peek = instructions.iter().peekable();

        while instr_peek.peek().is_some() {
            let instr = instr_peek.next().unwrap();
            match instr {
                Instruction::Op(op) => match op {
                    Operations::AddEntity => {
                        let const_idx = Self::get_constant_idx(&mut instr_peek);
                        let val = constants[const_idx as usize].unwrap_loaded_asset();
                        if let Some(mesh_handle) = self.set_la_data(val, queue) {
                            res.push(RenderUpdateDelta::AssetGPULoaded(mesh_handle));
                        }
                    }
                    _ => todo!(),
                },
                Instruction::Byte(byte) => {}
                Instruction::ConstIdx(idx) => {}
            }
        }

        res
    }

    fn doo_wop(instruction: &Instruction, stack: &mut Vec<VMValue>, op: Operations) {
        match op {
            Operations::AddEntity => {}
            Operations::MoveEntity => {
                todo!()
            }
        }
    }
}
