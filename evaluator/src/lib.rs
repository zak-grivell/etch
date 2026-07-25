// use ast::{Expression, Statement};
// use std::{cell::RefCell, collections::HashMap, rc::Rc};

// #[cfg(test)]
// mod test;

// #[derive(Debug)]
// pub struct Node {
//     connections: Vec<Connection>,
// }

// #[derive(Debug)]
// pub struct Connection {
//     a: Rc<RefCell<Node>>,
//     b: Rc<RefCell<Node>>,

//     name: String,
// }

// #[derive(Debug)]
// pub enum Value<'src> {
//     Object(HashMap<&'src str, Rc<Value<'src>>>),
//     String(&'src str),
//     Lambda {
//         params: HashMap<&'src str, Option<&'src str>>,
//         body: Vec<Statement<'src>>,
//         scope: Scope<'src>,
//     },
//     Array(Vec<Rc<Value<'src>>>),
//     Some(Rc<Value<'src>>),
//     Number {
//         value: f64,
//         unit: Option<&'src str>,
//     },
//     Boolean(bool),
//     Node(Rc<RefCell<Node>>),
//     None,
// }

// pub fn evauluate_expression<'src>(expr: Expression<'src>, scope: &Scope<'src>) -> Rc<Value<'src>> {
//     let e: Rc<Value> = match expr {
//         Expression::Number { value, unit } => Value::Number { value, unit }.into(),
//         Expression::String(str) => Value::String(str).into(),
//         Expression::Object(mp) => Value::Object(
//             mp.into_iter()
//                 .map(|(k, v)| (k, evauluate_expression(v, scope)))
//                 .collect(),
//         )
//         .into(),
//         Expression::Array(arr) => Value::Array(
//             arr.into_iter()
//                 .map(|expr| evauluate_expression(expr, scope))
//                 .collect(),
//         )
//         .into(),
//         Expression::Lambda { params, body } => {
//             let sc = scope.varibles.borrow().clone();

//             Value::Lambda {
//                 params,
//                 body,
//                 scope: Scope {
//                     varibles: Rc::new(RefCell::new(sc)),
//                 },
//             }
//         }
//         .into(),
//         Expression::Boolean(b) => Value::Boolean(b).into(),

//         Expression::None => Value::None.into(),
//         Expression::Some(v) => Value::Some(evauluate_expression(*v, scope)).into(),

//         Expression::Match { value, conds } => todo!(),

//         Expression::Block { body } => evaluate_statements(&body, scope),

//         Expression::Ident(name) => scope
//             .varibles
//             .borrow()
//             .get(name)
//             .unwrap_or_else(|| panic!("{} does not exist in scope", name))
//             .clone(),

//         Expression::Call { expression, args } => {
//             let lambda = evauluate_expression(*expression, scope);
//             let Value::Lambda {
//                 params,
//                 body,
//                 scope: lambda_scope,
//             } = lambda.as_ref()
//             else {
//                 panic!("bad eval");
//             };

//             evaluate_statements(body, lambda_scope)
//         }

//         Expression::ObjectAcess { expr, field } => {
//             let val = evauluate_expression(*expr, scope);
//             let Value::Object(obj) = val.as_ref() else {
//                 panic!("Not an object while trying to access object");
//             };
//             obj.get(field).unwrap_or(&Rc::new(Value::None)).clone()
//         }

//         Expression::Negate(expr) => {
//             let val = evauluate_expression(*expr, scope);
//             let Value::Number { value, unit } = val.as_ref() else {
//                 panic!("NAN when trying to negate");
//             };
//             Value::Number {
//                 value: -value,
//                 unit: *unit,
//             }
//             .into()
//         }

//         Expression::Flip(expr) => {
//             let val = evauluate_expression(*expr, scope);
//             let Value::Boolean(b) = val.as_ref() else {
//                 panic!("NAN when trying to flip");
//             };
//             Value::Boolean(!b).into()
//         }

//         Expression::Add(expr1, expr2) => {
//             let va = evauluate_expression(*expr1, scope);
//             let vb = evauluate_expression(*expr2, scope);
//             let Value::Number { value: a, unit } = va.as_ref() else {
//                 panic!("NAN add a")
//             };
//             let Value::Number { value: b, .. } = vb.as_ref() else {
//                 panic!("NAN add b")
//             };
//             Value::Number {
//                 value: a + b,
//                 unit: *unit,
//             }
//             .into()
//         }
//         Expression::Sub(expr1, expr2) => {
//             let va = evauluate_expression(*expr1, scope);
//             let vb = evauluate_expression(*expr2, scope);
//             let Value::Number { value: a, unit } = va.as_ref() else {
//                 panic!("NAN sub a")
//             };
//             let Value::Number { value: b, .. } = vb.as_ref() else {
//                 panic!("NAN sub b")
//             };
//             Value::Number {
//                 value: a - b,
//                 unit: *unit,
//             }
//             .into()
//         }
//         Expression::Mul(expr1, expr2) => {
//             let va = evauluate_expression(*expr1, scope);
//             let vb = evauluate_expression(*expr2, scope);
//             let Value::Number { value: a, unit } = va.as_ref() else {
//                 panic!("NAN mul a")
//             };
//             let Value::Number { value: b, .. } = vb.as_ref() else {
//                 panic!("NAN mul b")
//             };
//             Value::Number {
//                 value: a * b,
//                 unit: *unit,
//             }
//             .into()
//         }
//         Expression::Div(expr1, expr2) => {
//             let va = evauluate_expression(*expr1, scope);
//             let vb = evauluate_expression(*expr2, scope);
//             let Value::Number { value: a, unit } = va.as_ref() else {
//                 panic!("NAN div a")
//             };
//             let Value::Number { value: b, .. } = vb.as_ref() else {
//                 panic!("NAN div b")
//             };
//             Value::Number {
//                 value: a / b,
//                 unit: *unit,
//             }
//             .into()
//         }

//         Expression::Union(expr1, expr2) => {
//             let ra = evauluate_expression(*expr1, scope);
//             let rb = evauluate_expression(*expr2, scope);
//             let Value::Object(a) = ra.as_ref() else {
//                 panic!("not object union a")
//             };
//             let Value::Object(b) = rb.as_ref() else {
//                 panic!("not object union b")
//             };
//             let mut out = HashMap::new();
//             for (k, v) in a.iter() {
//                 out.insert(*k, v.clone());
//             }
//             for (k, v) in b.iter() {
//                 out.insert(*k, v.clone());
//             }
//             Value::Object(out).into()
//         }
//         Expression::Wire(expr1, expr2) => {
//             let ra = evauluate_expression(*expr1, scope);
//             let rb = evauluate_expression(*expr2, scope);
//             let Value::Object(a) = ra.as_ref() else {
//                 panic!("not object wire a")
//             };
//             let Value::Object(b) = rb.as_ref() else {
//                 panic!("not object wire b")
//             };
//             let mut out = HashMap::new();
//             for (k, v) in a.iter() {
//                 out.insert(*k, v.clone());
//             }
//             for (k, v) in b.iter() {
//                 if !out.contains_key(k) {
//                     panic!("Cannot wire in")
//                 }
//                 out.insert(*k, v.clone());
//             }
//             Value::Object(out).into()
//         }
//     };

//     e
// }

// pub fn evaluate_statements<'src>(
//     statements: &Vec<Statement<'src>>,
//     scope: &Scope<'src>,
// ) -> Rc<Value<'src>> {
//     for statement in statements {
//         match statement {
//             Statement::Definition { name, rhs } => {
//                 let result = evauluate_expression(*rhs.clone(), scope);
//                 scope.varibles.borrow_mut().insert(name, result);
//             }
//             Statement::Expression(expr) => {
//                 println!("Not evaluated");
//             }
//             Statement::Return { value } => return evauluate_expression(*value.clone(), scope),

//             Statement::TypeDefinition { name, rhs } => todo!(),
//         };
//     }

//     Value::None.into()
// }

// #[derive(Clone, Default, Debug)]
// pub struct Scope<'src> {
//     varibles: Rc<RefCell<HashMap<&'src str, Rc<Value<'src>>>>>,
//     // types: Rc<RefCell<HashMap<&'src str, Rc<Value<'src>>>>>,
// }

// impl<'a> Scope<'a> {
//     pub fn new() -> Scope<'a> {
//         Scope {
//             varibles: Rc::new(RefCell::new(HashMap::new())),
//         }
//     }
// }
