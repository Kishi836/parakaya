//Value.rs: initialization for the 4 types of values that can be stored in the stack of the virtual machine.

#[derive(Copy, Clone, Debug, PartialEq)] // enables the 4 capablities: Copy (a=b), Clone (a.clone()), Debug (printable), PartialEq (a==b)
pub enum Value {
    //Public type
    I32(i32), //i32 is a 32-bit signed integer
    I64(i64), //i64 is a 64-bit signed integer
    F32(f32), //f32 is a 32-bit floating point number
    F64(f64), //f64 is a 64-bit floating point number
}
