# STM32F407 flight computer

This is a board-level Etch example built around an STM32F407VET6. It includes:

- an SX1278 LoRa radio on SPI1;
- a MAX3485 half-duplex RS-485 port on USART2;
- a u-blox NEO-M8N GPS on USART3;
- ADXL343 accelerometer, BMP280 barometer, and LIS3MDL magnetometer on I2C1;
- 5 V input, 3.3 V regulation, MCU analog filtering, local decoupling, SWD, reset, and boot configuration;
- I2C pull-ups, sensor address straps, radio/GPS antenna connectors, and RS-485 termination and fail-safe biasing.

The design is split into named schematic sections, while labelled nets join the sections. Generate the outputs from the repository root:

```sh
cargo run -p cli -- check examples/flight_computer/main.etch
cargo run -p cli -- export schematic examples/flight_computer/main.etch --format svg --output flight_computer.svg
cargo run -p cli -- export schematic examples/flight_computer/main.etch --format kicad --output flight_computer.kicad_sch
cargo run -p cli -- export pcb examples/flight_computer/main.etch --format svg --output flight_computer_pcb.svg
cargo run -p cli -- export pcb examples/flight_computer/main.etch --format kicad --output flight_computer.kicad_pcb
```

This is a substantial reference design, not a production-ready avionics design. Review RF matching, antenna layout, ESD/surge protection, regulator thermal limits, sensor placement, grounding, connector pinout, and environmental requirements before hardware manufacture.
