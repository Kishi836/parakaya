use crate::instr::Instr;
use std::collections::HashMap;

/// Where a structured opener (`Block`/`Loop`/`If`) ends, and where an `If`'s else-arm starts.
#[derive(Copy, Clone, Debug)]
pub struct JumpInfo {
    pub end_pc: usize,
    pub else_pc: Option<usize>,
}

/// Prescan a function body: map every opener's pc to its matching `End` (and `Else`).
/// Panics on malformed nesting — the decoder only produces well-formed bodies.
pub fn compute(instrs: &[Instr]) -> HashMap<usize, JumpInfo> {
    let mut map = HashMap::new();
    let mut openers: Vec<usize> = Vec::new();
    for (pc, ins) in instrs.iter().enumerate() {
        match ins {
            Instr::Block | Instr::Loop | Instr::If => openers.push(pc),
            Instr::Else => {
                let opener = *openers.last().expect("else without if");
                map.insert(
                    opener,
                    JumpInfo {
                        end_pc: usize::MAX,
                        else_pc: Some(pc),
                    },
                );
            }
            Instr::End => {
                if let Some(opener) = openers.pop() {
                    let else_pc = map.get(&opener).and_then(|j| j.else_pc);
                    map.insert(
                        opener,
                        JumpInfo {
                            end_pc: pc,
                            else_pc,
                        },
                    );
                }
                // an End with no opener terminates the function body: ignore
            }
            _ => {}
        }
    }
    map
}
