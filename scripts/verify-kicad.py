#!/usr/bin/env python3
"""Verify native schematic connectivity and real-footprint boards with KiCad DRC."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
KICAD = os.environ.get('KICAD_CLI') or shutil.which('kicad-cli') or '/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli'

def run(*args):
    subprocess.run(args, cwd=ROOT, check=True)

def verify(name, source, expected, directory):
    sch, board, xml, report = [directory / (name + suffix) for suffix in ['.kicad_sch', '.kicad_pcb', '.xml', '.json']]
    for kind, output in [('schematic', sch), ('pcb', board)]:
        run('cargo', 'run', '--quiet', '-p', 'cli', '--bin', 'etch', '--', 'export', kind, str(source), '--format', 'kicad', '--output', str(output))
    run(KICAD, 'sch', 'export', 'netlist', str(sch), '--format', 'kicadxml', '-o', str(xml))
    nets = [set((node.attrib['ref'], node.attrib['pin']) for node in net.findall('node')) for net in ET.parse(xml).findall('.//nets/net')]
    for group in expected:
        assert any(set(group).issubset(net) for net in nets), (name, group, nets)
    run(KICAD, 'pcb', 'drc', str(board), '--format', 'json', '--output', str(report))
    result = json.loads(report.read_text())
    assert not result['violations'], (name, result['violations'])
    assert not result['unconnected_items'], (name, result['unconnected_items'])
    print(name + ': verified connectivity; zero DRC violations and unconnected items')

with tempfile.TemporaryDirectory(prefix='etch-kicad-') as temporary:
    directory = Path(temporary)
    verify('divider', ROOT / 'examples/pcb_voltage_divider.etch', [[('V1','1'),('R1','1')],[('R1','2'),('R2','1')],[('R2','2'),('V1','2')]], directory)
    led = directory / 'led.etch'
    led.write_text('''
from "std/sources.etch" import { VoltageSource };
from "std/passive.etch" import { Resistor };
from "std/analog.etch" import { Led };
pcb_config(width:30,height:20,layers:2,min_trace_width:0.25,clearance:0.2);
let supply = VoltageSource(value:5);
let resistor = Resistor(resistance:1000);
let led = Led(forward_voltage:2,on_resistance:10,off_resistance:1000000);
supply.positive <- resistor.a; resistor.b <- led.anode; led.cathode <- ground
''')
    verify('led',led,[[('R1','2'),('D1','2')],[('D1','1'),('V1','2')]],directory)
    shared = directory / 'shared.etch'
    shared.write_text('''
pcb_config(width:20,height:20,layers:2,min_trace_width:0.25,clearance:0.2);
use_symbol(kind:"resistor",ports:{a:ground,b:ground},kicad:{symbol:"Device:R",footprint:"Resistor_SMD:R_0603_1608Metric",pins:{a:"1",b:"2"}})
''')
    verify('shared-node',shared,[[('R1','1'),('R1','2')]],directory)
