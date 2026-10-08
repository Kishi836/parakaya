#[derive(Clone, Debug, PartialEq, Eq)] //enables the 4 capablities: Clone (a.clone()), Debug (printable), PartialEq (a==b), Eq (a==b) but also requires that the type is comparable for equality
pub enum Trap {
    Unreachable, //infinite loop
    DivByZero, //division by zero
    OutOfBoundsMemory, //memory runs out
    StackUnderflow, //stack is empty (pop() on empty stack)
    TypeMismatch, //type of value is not compatible with the operation (int and char cant be compared)
    IntegerOverflow,
}
