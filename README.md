A rust based language for circuit design, simulation, and testing

lots of realy smart things have been developed in the software world like automated testing, mocking and functions. What if we could apply these concepts to circuit design? this is the motivation behind etch.

proposed DSL
```js

module VoltageDivider (r_a: Number, r_b: Number) {
  let { a: V_IN, b:V_MID } = Resistor(r_a)
  let { b: V_OUT } = Resistor(r_b) { a: V_MID }
  
  return {
    V_MID,
    V_IN,
    V_OUT
  };
}

module XOR {
  let { x, y, a  } = AND;
  let { z: b} = OR { x, y };
  let { z } = OR { a, b };
  
  return {
    x, y, z
  }
}

```


I want to investigate pipes for when small values tho
