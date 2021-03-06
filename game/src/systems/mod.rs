// TODO(jhurstwright): Replace with no_std hashmap
use std::collections::HashMap;

use glam::Vec2;
use quad_rand as qrand;

use crate::components;
use crate::events;
use crate::events::Event;
use crate::graphics;
use crate::graphics::renderer;

pub struct GameWorld {
    pub world: hecs::World,
    pub events: Vec<events::Event>,
    pub camera: components::Camera2D,
}

pub fn create_snake_system(game_world: &mut GameWorld) {
    let GameWorld { world, .. } = game_world;
    let ahead = world.spawn((
        components::Snake,
        components::Position(Vec2::new(0., 0.)),
        components::Velocity(Vec2::new(0., 1.)),
        components::HeadDirection::default(),
        components::Material("Snake".into()),
        components::Mesh("Snake".into()),
    ));
    let tail = components::Tail { segment: 1, ahead };

    world.spawn((
        tail,
        components::Material("Tail".into()),
        components::Mesh("Tail".into()),
        components::Collision::snake(),
        components::Position(Vec2::new(0., -1.)),
    ));
}

pub fn update_input(game_world: &mut GameWorld, input: &components::Input) {
    let GameWorld { world, .. } = game_world;
    for (_, (vel, dir, _)) in &mut world.query::<(
        &components::Velocity,
        &mut components::HeadDirection,
        &components::Snake,
    )>() {
        if let Some(d) = input.direction() {
            if d.velocity() != vel.0 * -1. {
                dir.0 = d;
            }
        }
    }
}

pub fn add_food_system(game_world: &mut GameWorld) {
    let GameWorld { world, .. } = game_world;
    let snake_count = world.query::<&components::Food>().iter().count();
    if snake_count >= 10 {
        return;
    }

    let x = qrand::gen_range(-24, 24);
    let y = qrand::gen_range(-15, 15);
    let pos = components::Position(Vec2::new(x as f32, y as f32));
    world.spawn((
        pos,
        components::Collision::food(),
        components::Food,
        components::Material("Food".into()),
        components::Mesh("Food".into()),
    ));
}

pub fn update_velocity_direction(game_world: &mut GameWorld) {
    let GameWorld { world, .. } = game_world;
    for (_, (velocity, direction)) in
        &mut world.query::<(&mut components::Velocity, &components::HeadDirection)>()
    {
        velocity.0 = direction.0.velocity();
    }
}

pub fn movement_system(game_world: &mut GameWorld) {
    let GameWorld { world, .. } = game_world;
    for (_, (pos, velocity)) in
        &mut world.query::<(&mut components::Position, &components::Velocity)>()
    {
        pos.0 = pos.0 + velocity.0;
    }
}

pub fn tail_movement_system(game_world: &mut GameWorld) {
    let GameWorld { world, .. } = game_world;
    let foo: HashMap<hecs::Entity, glam::Vec2> = world
        .query::<&components::Tail>()
        .iter()
        .map(|(_, tail)| {
            let pos = {
                world
                    .get::<components::Position>(tail.ahead)
                    .expect("All Ahead should has Positions")
                    .0
            };
            (tail.ahead.clone(), pos)
        })
        .collect();
    for (_, (tail, position)) in
        &mut world.query::<(&components::Tail, &mut components::Position)>()
    {
        let new_pos = foo[&tail.ahead];
        position.0 = new_pos;
    }
}

pub fn update_score_system(
    game_world: &mut GameWorld,
    score: &mut i32,
    cmds: &mut Vec<renderer::RenderAssetCommands>,
) {
    let GameWorld { world, events, .. } = game_world;
    for event in events {
        match event {
            Event::SnakeEatFood { .. } => {
                *score += 1;
                for (_, (text, _score)) in
                    &mut world.query::<(&mut components::Text, &components::Score)>()
                {
                    let cmd = text.update_text(format!("Score:  {}", score));
                    cmds.push(cmd);
                }
            }
            Event::GameOver => {
                *score = 0;
                for (_, (text, _score)) in
                    &mut world.query::<(&mut components::Text, &components::Score)>()
                {
                    let cmd = text.update_text(format!("Score:  {}", score));
                    cmds.push(cmd);
                }
            }
            _ => {}
        }
    }
}

pub fn despawn_food_system(game_world: &mut GameWorld) {
    let GameWorld { world, events, .. } = game_world;
    for event in events {
        match event {
            Event::SnakeEatFood { entity, pos: _pos } => {
                world
                    .despawn(*entity)
                    .expect("Food Eating System should not be destroying a non-existant Entity");
            }
            _ => {}
        }
    }
}

pub fn trigger_tail_spawn(game_world: &mut GameWorld) {
    let GameWorld { world, events, .. } = game_world;
    let mut events_to_push: Vec<Event> = Vec::new();
    for event in events.iter() {
        match event {
            Event::SnakeEatFood { .. } => {
                if let Some((ahead, (tail, pos))) = &world
                    .query::<(&components::Tail, &components::Position)>()
                    .iter()
                    .max_by_key(|(_, (tail, _))| tail.segment)
                {
                    events_to_push.push(Event::SpawnSnakeTail {
                        ahead: ahead.clone(),
                        pos: pos.0,
                        segment: tail.segment + 1,
                    })
                }
            }
            _ => {}
        }
    }
    events_to_push
        .iter()
        .for_each(|evt| events.push(evt.clone()));
}

pub fn spawn_tail_system(game_world: &mut GameWorld) {
    let GameWorld { world, events, .. } = game_world;
    for event in events.iter() {
        match event {
            Event::SpawnSnakeTail {
                ahead,
                pos,
                segment,
            } => {
                let tail = components::Tail {
                    segment: segment.clone(),
                    ahead: ahead.clone(),
                };

                world.spawn((
                    tail,
                    components::Material("Tail".into()),
                    components::Mesh("Tail".into()),
                    components::Collision::snake(),
                    components::Position(pos.clone()),
                ));
            }
            _ => {}
        }
    }
}

pub fn head_collision_system(game_world: &mut GameWorld) {
    let GameWorld { world, events, .. } = game_world;
    let (source_ent, source_pos): (hecs::Entity, Vec2) = match world
        .query::<(
            &components::Snake,
            &components::Position,
            &components::Velocity,
        )>()
        .iter()
        .map(|(ent, (_, pos, vel))| (ent, pos.0 + vel.0))
        .nth(0)
    {
        Some(it) => it,
        _ => return,
    };
    world
        .query::<(&components::Position, &components::Collision)>()
        .iter()
        .filter_map(|(ent, (target_pos, col))| {
            if target_pos.0 == source_pos {
                Some(Event::Collision {
                    target: ent,
                    source: source_ent,
                    pos: target_pos.0,
                    kind: col.kind,
                })
            } else {
                None
            }
        })
        .for_each(|event| events.push(event));
}

pub fn handle_collision_system(game_world: &mut GameWorld) {
    let (collsions, rest): (Vec<Event>, Vec<Event>) =
        game_world
            .events
            .iter()
            .cloned()
            .partition(|event| match event {
                Event::Collision { .. } => true,
                _ => false,
            });
    game_world.events = rest;
    collsions.iter().for_each(|collision| match collision {
        Event::Collision {
            kind: components::CollsionKind::Snake,
            ..
        } => game_world.events.push(Event::GameOver),
        Event::Collision {
            kind: components::CollsionKind::Food,
            target,
            ..
        } => {
            let entity = target.clone();
            let pos = game_world
                .world
                .get::<components::Position>(entity)
                .expect("Food should have components::Position");
            game_world
                .events
                .push(Event::SnakeEatFood { entity, pos: pos.0 });
        }
        _ => {}
    });
}

pub fn game_over_system(game_world: &mut GameWorld) -> bool {
    let GameWorld { world, events, .. } = game_world;

    let filter = events
        .iter()
        .filter(|event| match event {
            Event::GameOver => true,
            _ => false,
        })
        .nth(0);
    if let Some(_) = filter {
        world.clear();
        create_snake_system(game_world);
        return true;
    }
    false
}

pub fn gather_render_cmds(game_world: &mut GameWorld, renderer: &mut graphics::MainRenderer) {
    let GameWorld { world, .. } = game_world;
    let main_draw_commands = &mut renderer.main_render_target.commands;
    for (_, (mesh, material, pos)) in &mut world.query::<(
        &components::Mesh,
        &components::Material,
        &components::Position,
    )>() {
        main_draw_commands.push(renderer::RenderCommand::DrawMesh2D(renderer::DrawMesh2D {
            rotation: 0f32,
            material: material.0.clone(),
            mesh: mesh.0.clone(),
            position: pos.0,
        }));
    }
}

pub fn debug_render_cmds(game_world: &mut GameWorld, renderer: &mut graphics::MainRenderer) {
    let GameWorld { world, .. } = game_world;

    let debug_draw_commands = &mut renderer.debug_render_target.commands;
    for (_, (dir, pos)) in &mut world.query::<(&components::HeadDirection, &components::Position)>()
    {
        let vel = dir.0.velocity();
        let velocity = Vec2::new(vel.x, vel.y * -1.);
        let angle = velocity.angle_between(Vec2::new(1., 0.));
        debug_draw_commands.push(renderer::RenderCommand::DrawMesh2D(renderer::DrawMesh2D {
            material: "Arrow".into(),
            mesh: "Arrow".into(),
            position: vel + pos.0,
            rotation: angle,
        }));
    }
}

pub fn draw_text(game_world: &mut GameWorld, renderer: &mut graphics::MainRenderer) {
    let GameWorld { world, .. } = game_world;

    let main_draw_commands = &mut renderer.main_render_target.commands;
    for (_, (text, pos)) in &mut world.query::<(&components::Text, &components::Position)>() {
        main_draw_commands.push(renderer::RenderCommand::DrawFont(renderer::DrawFont {
            text: text.text().clone(),
            font: "KenneyFuture".into(),
            position: pos.0,
        }));
    }
}
