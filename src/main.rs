use rltk::{GameState, Point, Rltk, RGB};
use specs::prelude::*;

mod components;
pub use components::*;
mod map;
pub use map::*;
mod player;
pub use player::*;
mod rect;
pub use rect::Rect;
mod visibility_system;
use visibility_system::VisibilitySystem;
mod monster_ai_system;
use monster_ai_system::MonsterAI;
mod map_indexing_system;
use map_indexing_system::MapIndexingSystem;
mod melee_combat_system;
use melee_combat_system::MeleeCombatSystem;
mod damage_system;
use damage_system::DamageSystem;
mod gamelog;
mod gui;

#[derive(PartialEq, Copy, Clone)]
pub enum RunState {
    AwaitingInput,
    PreRun,
    PlayerTurn,
    MonsterTurn,
    GameOver,
}

pub struct State {
    pub ecs: World,
}

impl State {
    fn run_systems(&mut self) {
        let mut vis = VisibilitySystem {};
        vis.run_now(&self.ecs);
        let mut mob = MonsterAI {};
        mob.run_now(&self.ecs);
        let mut mapindex = MapIndexingSystem {};
        mapindex.run_now(&self.ecs);
        let mut melee = MeleeCombatSystem {};
        melee.run_now(&self.ecs);
        let mut damage = DamageSystem {};
        damage.run_now(&self.ecs);
        self.ecs.maintain();
    }

    fn player_is_dead(&self) -> bool {
        let player = self.ecs.fetch::<Entity>();
        let stats = self.ecs.read_storage::<CombatStats>();
        match stats.get(*player) {
            Some(s) => s.hp < 1,
            None => true,
        }
    }

    fn restart(&mut self) {
        self.ecs.delete_all();
        spawn_world(&mut self.ecs);
    }
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut Rltk) {
        ctx.cls();
        let mut newrunstate;
        {
            let runstate = self.ecs.fetch::<RunState>();
            newrunstate = *runstate;
        }

        match newrunstate {
            RunState::PreRun => {
                self.run_systems();
                newrunstate = RunState::AwaitingInput;
            }
            RunState::AwaitingInput => {
                newrunstate = player_input(self, ctx);
            }
            RunState::PlayerTurn => {
                self.run_systems();
                newrunstate = RunState::MonsterTurn;
            }
            RunState::MonsterTurn => {
                self.run_systems();
                newrunstate = RunState::AwaitingInput;
            }
            RunState::GameOver => match ctx.key {
                Some(rltk::VirtualKeyCode::Y) | Some(rltk::VirtualKeyCode::Return) => {
                    self.restart();
                    newrunstate = RunState::PreRun;
                }
                _ => {}
            },
        }

        if newrunstate != RunState::GameOver
            && newrunstate != RunState::PreRun
            && self.player_is_dead()
        {
            newrunstate = RunState::GameOver;
        }

        {
            let mut runwriter = self.ecs.write_resource::<RunState>();
            *runwriter = newrunstate;
        }
        damage_system::delete_the_dead(&mut self.ecs);

        draw_map(&self.ecs, ctx);

        let positions = self.ecs.read_storage::<Position>();
        let renderables = self.ecs.read_storage::<Renderable>();
        let map = self.ecs.fetch::<Map>();

        for (pos, render) in (&positions, &renderables).join() {
            let idx = map.xy_idx(pos.x, pos.y);
            if map.visible_tiles[idx] {
                ctx.set(pos.x, pos.y, render.fg, render.bg, render.glyph)
            }
        }

        gui::draw_ui(&self.ecs, ctx);

        if newrunstate == RunState::GameOver {
            let fg = RGB::named(rltk::RED);
            let bg = RGB::named(rltk::BLACK);
            ctx.draw_box_double(20, 18, 39, 8, fg, bg);
            ctx.print_color_centered(20, fg, bg, "GAME OVER");
            ctx.print_color_centered(22, RGB::named(rltk::WHITE), bg, "You have died.");
            ctx.print_color_centered(24, RGB::named(rltk::YELLOW), bg, "Try again? (Y / Enter)");
        }
    }
}

fn main() -> rltk::BError {
    use rltk::RltkBuilder;
    let mut context = RltkBuilder::simple80x50()
        .with_title("RogusMaximusRusticus")
        .build()?;
    context.with_post_scanlines(true);
    let mut gs = State {
        ecs: World::new(),
        //runState : RunState::Running
    };
    //register stuff here!
    gs.ecs.register::<Position>();
    gs.ecs.register::<Renderable>();
    gs.ecs.register::<Player>();
    gs.ecs.register::<Viewshed>();
    gs.ecs.register::<Monster>();
    gs.ecs.register::<Name>();
    gs.ecs.register::<BlocksTile>();
    gs.ecs.register::<CombatStats>();
    gs.ecs.register::<WantsToMelee>();
    gs.ecs.register::<SufferDamage>();

    spawn_world(&mut gs.ecs);

    rltk::main_loop(context, gs)
}

fn spawn_world(ecs: &mut World) {
    let map: Map = Map::new_map_rooms_and_corridors();
    let (player_x, player_y) = map.rooms[0].center();

    let player_entity = ecs
        .create_entity()
        .with(Position {
            x: player_x,
            y: player_y,
        })
        .with(Renderable {
            glyph: rltk::to_cp437('@'),
            fg: RGB::named(rltk::YELLOW),
            bg: RGB::named(rltk::BLACK),
        })
        .with(Player {})
        .with(Viewshed {
            visible_tiles: Vec::new(),
            range: 8,
            dirty: true,
        })
        .with(Name {
            name: "Paul".to_string(),
        })
        .with(CombatStats {
            max_hp: 50,
            defense: 2,
            hp: 30,
            power: 5,
        })
        .build();

    let mut rng = rltk::RandomNumberGenerator::new();
    for (i, room) in map.rooms.iter().skip(1).enumerate() {
        let (x, y) = room.center();

        let glyph: rltk::FontCharType;
        let name: String;
        let roll = rng.roll_dice(1, 2);
        match roll {
            1 => {
                glyph = rltk::to_cp437('g');
                name = "Göbelin".to_string();
            }
            _ => {
                glyph = rltk::to_cp437('o');
                name = "Orkus".to_string();
            }
        }

        ecs.create_entity()
            .with(Position { x, y })
            .with(Renderable {
                glyph: glyph,
                fg: RGB::named(rltk::RED),
                bg: RGB::named(rltk::BLACK),
            })
            .with(Viewshed {
                visible_tiles: Vec::new(),
                range: 8,
                dirty: true,
            })
            .with(Monster {})
            .with(Name {
                name: format!("{} #{}", &name, i),
            })
            .with(BlocksTile {})
            .with(CombatStats {
                max_hp: 16,
                defense: 1,
                hp: 16,
                power: 4,
            })
            .build();
    }

    ecs.insert(map);
    ecs.insert(Point::new(player_x, player_y));
    ecs.insert(player_entity);
    ecs.insert(RunState::PreRun);
    ecs.insert(gamelog::GameLog {
        entries: vec!["You have entered Der WienerSchnitzel".to_string()],
    });
}
