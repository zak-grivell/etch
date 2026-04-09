use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Graph {
    label: String,
    points: Arc<Mutex<VecDeque<[f64; 2]>>>,
    t: Arc<Mutex<f64>>,
}

impl Graph {
    fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            points: Arc::new(Mutex::new(VecDeque::with_capacity(500))),
            t: Arc::new(Mutex::new(0.0)),
        }
    }

    pub fn add(&self, y: f64) {
        let mut t = self.t.lock().unwrap();
        let x = *t;
        *t += 1.0;
        drop(t);
        let mut pts = self.points.lock().unwrap();
        if pts.len() >= 500 {
            pts.pop_front();
        }
        pts.push_back([x, y]);
    }
}

// ── App that holds all plots ──────────────────────────────────────────────────

pub struct GraphApp {
    graphs: Vec<Graph>,
}

impl GraphApp {
    pub fn new() -> Self {
        Self { graphs: vec![] }
    }

    /// Register a new plot and return its handle for use in your sim thread.
    pub fn plot(&mut self, label: &str) -> Graph {
        let g = Graph::new(label);
        self.graphs.push(g.clone());
        g
    }

    /// Blocks the main thread — call this last.
    pub fn run(self) {
        eframe::run_native(
            "graph",
            eframe::NativeOptions::default(),
            Box::new(|_cc| Ok(Box::new(PlotAppInner(self.graphs)))),
        )
        .unwrap();
    }
}

struct PlotAppInner(Vec<Graph>);

impl eframe::App for PlotAppInner {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        Plot::new("plot")
            .height(ui.available_height())
            .show(ui, |p| {
                for graph in &self.0 {
                    let pts: Vec<[f64; 2]> = graph.points.lock().unwrap().iter().copied().collect();
                    p.line(Line::new(&graph.label, PlotPoints::from(pts)).width(2.0));
                }
            });
        ui.ctx().request_repaint();
    }
}
// ── Your sim ──────────────────────────────────────────────────────────────────

// pub fn main() {
//     let mut app = GraphApp::new();
//     let voltage = app.plot("Voltage");
//     let current = app.plot("Current");

//     thread::spawn(move || {
//         // ... your node/edge setup ...
//         loop {
//             // supply_net.predict_voltage(); etc.
//             voltage.add(supply_net.voltage());
//             current.add(mid1.current());
//             thread::sleep(Duration::from_millis(50));
//         }
//     });

//     app.run(); // must be last — blocks main thread
// }
