use bevy::prelude::*;
use bevy::{color::palettes, math::Vec4Swizzles};
use bevy_ecs_tilemap::helpers::hex_grid::neighbors::{HexDirection, HexNeighbors};
use bevy_ecs_tilemap::prelude::*;
mod helpers;
use helpers::camera::movement as camera_movement;

// Press SPACE to change map type. Hover over a tile to highlight its label (red) and those of its
// neighbors (blue). Press and hold one of keys 0-5 to mark the neighbor in that direction (green).

// You can increase the MAP_SIDE_LENGTH, in order to test that mouse picking works for larger maps,
// but just make sure that you run in release mode (`cargo run --release --example mouse_to_tile`)
// otherwise things might be too slow.
const MAP_SIDE_LENGTH_X: u32 = 4;
const MAP_SIDE_LENGTH_Y: u32 = 4;

const TILE_SIZE_HEX_ROW: TilemapTileSize = TilemapTileSize { x: 50.0, y: 58.0 };
const TILE_SIZE_HEX_COL: TilemapTileSize = TilemapTileSize { x: 58.0, y: 50.0 };
const GRID_SIZE_HEX_ROW: TilemapGridSize = TilemapGridSize { x: 50.0, y: 58.0 };
const GRID_SIZE_HEX_COL: TilemapGridSize = TilemapGridSize { x: 58.0, y: 50.0 };

#[derive(Deref, Resource)]
pub struct TileHandleHexRow(Handle<Image>);

#[derive(Deref, Resource)]
pub struct TileHandleHexCol(Handle<Image>);

impl FromWorld for TileHandleHexCol {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self(asset_server.load("bw-tile-hex-col.png"))
    }
}
impl FromWorld for TileHandleHexRow {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        Self(asset_server.load("bw-tile-hex-row.png"))
    }
}

// Generates the initial tilemap, which is a hex grid.
fn spawn_tilemap(mut commands: Commands, tile_handle_hex_row: Res<TileHandleHexRow>) {
    commands.spawn(Camera2d);

    let map_size = TilemapSize {
        x: MAP_SIDE_LENGTH_X,
        y: MAP_SIDE_LENGTH_Y,
    };

    let mut tile_storage = TileStorage::empty(map_size);
    let tilemap_entity = commands.spawn_empty().id();
    let tilemap_id = TilemapId(tilemap_entity);

    fill_tilemap(
        TileTextureIndex(0),
        map_size,
        tilemap_id,
        &mut commands,
        &mut tile_storage,
    );

    let tile_size = TILE_SIZE_HEX_ROW;
    let grid_size = GRID_SIZE_HEX_ROW;
    let map_type = TilemapType::Hexagon(HexCoordSystem::Row);

    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size,
        size: map_size,
        storage: tile_storage,
        texture: TilemapTexture::Single(tile_handle_hex_row.clone()),
        tile_size,
        map_type,
        anchor: TilemapAnchor::Center,
        ..Default::default()
    });
}

#[derive(Component)]
struct TileLabel(Entity);

// Generates tile position labels of the form: `(tile_pos.x, tile_pos.y)`
fn spawn_tile_labels(
    mut commands: Commands,
    tilemap_q: Query<(
        &Transform,
        &TilemapType,
        &TilemapSize,
        &TilemapGridSize,
        &TilemapTileSize,
        &TileStorage,
        &TilemapAnchor,
    )>,
    tile_q: Query<&mut TilePos>,
) {
    for (map_transform, map_type, map_size, grid_size, tile_size, tilemap_storage, anchor) in
        tilemap_q.iter()
    {
        for tile_entity in tilemap_storage.iter().flatten() {
            let tile_pos = tile_q.get(*tile_entity).unwrap();
            let tile_center = tile_pos
                .center_in_world(map_size, grid_size, tile_size, map_type, anchor)
                .extend(1.0);
            let transform = *map_transform * Transform::from_translation(tile_center);

            let label_entity = commands
                .spawn((
                    Text2d::new(format!("{},{}", tile_pos.x, tile_pos.y)),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::BLACK),
                    TextLayout::new_with_justify(JustifyText::Center),
                    transform,
                ))
                .id();
            commands
                .entity(*tile_entity)
                .insert(TileLabel(label_entity));
        }
    }
}

#[derive(Component)]
pub struct MapTypeLabel;

// Generates the map type label: e.g. `Square { diagonal_neighbors: false }`
fn spawn_map_type_label(
    mut commands: Commands,
    windows: Query<&Window>,
    map_type_q: Query<&TilemapType>,
) {
    for window in windows.iter() {
        for map_type in map_type_q.iter() {
            // Place the map type label somewhere in the top left side of the screen
            let transform = Transform {
                translation: Vec2::new(-0.5 * window.width() / 2.0, 0.8 * window.height() / 2.0)
                    .extend(1.0),
                ..Default::default()
            };
            commands.spawn((
                Text2d::new(format!("{map_type:?}")),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::BLACK),
                TextLayout::new_with_justify(JustifyText::Center),
                transform,
                MapTypeLabel,
            ));
        }
    }
}

// Swaps the map type, when user presses SPACE
#[allow(clippy::too_many_arguments)]
fn swap_map_type(
    mut tilemap_query: Query<(
        &Transform,
        &TilemapSize,
        &mut TilemapType,
        &mut TilemapGridSize,
        &mut TilemapTexture,
        &mut TilemapTileSize,
        &TilemapAnchor,
    )>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    tile_label_q: Query<
        (&TileLabel, &TilePos),
        (With<TileLabel>, Without<MapTypeLabel>, Without<TilemapType>),
    >,
    mut map_type_label_q: Query<&mut Text2d, With<MapTypeLabel>>,
    mut transform_q: Query<&mut Transform, Without<TilemapType>>,
    tile_handle_hex_row: Res<TileHandleHexRow>,
    tile_handle_hex_col: Res<TileHandleHexCol>,
) {
    if keyboard_input.just_pressed(KeyCode::Space) {
        for (
            map_transform,
            map_size,
            mut map_type,
            mut grid_size,
            mut map_texture,
            mut tile_size,
            anchor,
        ) in tilemap_query.iter_mut()
        {
            match map_type.as_ref() {
                TilemapType::Hexagon(HexCoordSystem::Row) => {
                    *map_type = TilemapType::Hexagon(HexCoordSystem::RowEven);
                }
                TilemapType::Hexagon(HexCoordSystem::RowEven) => {
                    *map_type = TilemapType::Hexagon(HexCoordSystem::RowOdd);
                }
                TilemapType::Hexagon(HexCoordSystem::RowOdd) => {
                    *map_type = TilemapType::Hexagon(HexCoordSystem::Column);
                    *map_texture = TilemapTexture::Single((*tile_handle_hex_col).clone());
                    *tile_size = TILE_SIZE_HEX_COL;
                    *grid_size = GRID_SIZE_HEX_COL;
                }
                TilemapType::Hexagon(HexCoordSystem::Column) => {
                    *map_type = TilemapType::Hexagon(HexCoordSystem::ColumnEven);
                }
                TilemapType::Hexagon(HexCoordSystem::ColumnEven) => {
                    *map_type = TilemapType::Hexagon(HexCoordSystem::ColumnOdd);
                }
                TilemapType::Hexagon(HexCoordSystem::ColumnOdd) => {
                    *map_type = TilemapType::Hexagon(HexCoordSystem::Row);
                    *map_texture = TilemapTexture::Single((*tile_handle_hex_row).clone());
                    *tile_size = TILE_SIZE_HEX_ROW;
                    *grid_size = GRID_SIZE_HEX_ROW;
                }
                _ => unreachable!(),
            }

            for (label, tile_pos) in tile_label_q.iter() {
                if let Ok(mut tile_label_transform) = transform_q.get_mut(label.0) {
                    let tile_center = tile_pos
                        .center_in_world(map_size, &grid_size, &tile_size, &map_type, anchor)
                        .extend(1.0);
                    *tile_label_transform =
                        *map_transform * Transform::from_translation(tile_center);
                }
            }

            for mut label_text in map_type_label_q.iter_mut() {
                label_text.0 = format!("{:?}", map_type.as_ref());
            }
        }
    }
}

#[derive(Component)]
struct Hovered;

#[derive(Resource)]
pub struct CursorPos(Vec2);
impl Default for CursorPos {
    fn default() -> Self {
        // Initialize the cursor pos at some far away place. It will get updated
        // correctly when the cursor moves.
        Self(Vec2::new(-1000.0, -1000.0))
    }
}

// We need to keep the cursor position updated based on any `CursorMoved` events.
pub fn update_cursor_pos(
    camera_q: Query<(&GlobalTransform, &Camera)>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    for cursor_moved in cursor_moved_events.read() {
        // To get the mouse's world position, we have to transform its window position by
        // any transforms on the camera. This is done by projecting the cursor position into
        // camera space (world space).
        for (cam_t, cam) in camera_q.iter() {
            if let Ok(pos) = cam.viewport_to_world_2d(cam_t, cursor_moved.position) {
                *cursor_pos = CursorPos(pos);
            }
        }
    }
}

// This is where we check which tile the cursor is hovered over.
fn hover_highlight_tile_label(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    tilemap_q: Query<(
        &TilemapSize,
        &TilemapGridSize,
        &TilemapTileSize,
        &TilemapType,
        &TileStorage,
        &Transform,
        &TilemapAnchor,
    )>,
    highlighted_tiles_q: Query<Entity, With<Hovered>>,
    tile_label_q: Query<&TileLabel>,
    mut text_q: Query<&mut TextColor>,
) {
    // Un-highlight any previously highlighted tile labels.
    for highlighted_tile_entity in highlighted_tiles_q.iter() {
        if let Ok(label) = tile_label_q.get(highlighted_tile_entity) {
            if let Ok(mut text_color) = text_q.get_mut(label.0) {
                text_color.0 = Color::BLACK;
                commands.entity(highlighted_tile_entity).remove::<Hovered>();
            }
        }
    }

    for (map_size, grid_size, tile_size, map_type, tile_storage, map_transform, anchor) in
        tilemap_q.iter()
    {
        // Grab the cursor position from the `Res<CursorPos>`
        let cursor_pos: Vec2 = cursor_pos.0;
        // We need to make sure that the cursor's world position is correct relative to the map
        // due to any map transformation.
        let cursor_in_map_pos: Vec2 = {
            // Extend the cursor_pos vec2 by 0.0 and 1.0
            let cursor_pos = Vec4::from((cursor_pos, 0.0, 1.0));
            let cursor_in_map_pos = map_transform.compute_matrix().inverse() * cursor_pos;
            cursor_in_map_pos.xy()
        };
        // Once we have a world position we can transform it into a possible tile position.
        if let Some(tile_pos) = TilePos::from_world_pos(
            &cursor_in_map_pos,
            map_size,
            grid_size,
            tile_size,
            map_type,
            anchor,
        ) {
            // Highlight the relevant tile's label
            if let Some(tile_entity) = tile_storage.get(&tile_pos) {
                if let Ok(label) = tile_label_q.get(tile_entity) {
                    if let Ok(mut text_color) = text_q.get_mut(label.0) {
                        text_color.0 = palettes::tailwind::RED_600.into();
                        commands.entity(tile_entity).insert(Hovered);
                    }
                }
            }
        }
    }
}

#[derive(Component)]
struct NeighborHighlight;

// Highlight neighbor tiles of hovered tile
#[allow(clippy::too_many_arguments)]
fn highlight_neighbor_label(
    mut commands: Commands,
    tilemap_query: Query<(&TilemapType, &TilemapSize, &TileStorage)>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    highlighted_tiles_q: Query<Entity, With<NeighborHighlight>>,
    hovered_tiles_q: Query<&TilePos, With<Hovered>>,
    tile_label_q: Query<&TileLabel>,
    mut text_q: Query<&mut TextColor>,
) {
    // Un-highlight any previously highlighted tile labels.
    for highlighted_tile_entity in highlighted_tiles_q.iter() {
        if let Ok(label) = tile_label_q.get(highlighted_tile_entity) {
            if let Ok(mut text_color) = text_q.get_mut(label.0) {
                text_color.0 = Color::BLACK;
                commands
                    .entity(highlighted_tile_entity)
                    .remove::<NeighborHighlight>();
            }
        }
    }

    for (map_type, map_size, tile_storage) in tilemap_query.iter() {
        let hex_coord_sys = if let TilemapType::Hexagon(hex_coord_sys) = map_type {
            hex_coord_sys
        } else {
            continue;
        };

        for hovered_tile_pos in hovered_tiles_q.iter() {
            let neighboring_positions =
                HexNeighbors::get_neighboring_positions(hovered_tile_pos, map_size, hex_coord_sys);

            for neighbor_pos in neighboring_positions.iter() {
                // We want to ensure that the tile position lies within the tile map, so we do a
                // `checked_get`.
                if let Some(tile_entity) = tile_storage.checked_get(neighbor_pos) {
                    if let Ok(label) = tile_label_q.get(tile_entity) {
                        if let Ok(mut text_color) = text_q.get_mut(label.0) {
                            text_color.0 = palettes::tailwind::BLUE_600.into();
                            commands.entity(tile_entity).insert(NeighborHighlight);
                        }
                    }
                }
            }

            let selected_hex_direction = if keyboard_input.pressed(KeyCode::Digit0) {
                Some(HexDirection::Zero)
            } else if keyboard_input.pressed(KeyCode::Digit1) {
                Some(HexDirection::One)
            } else if keyboard_input.pressed(KeyCode::Digit2) {
                Some(HexDirection::Two)
            } else if keyboard_input.pressed(KeyCode::Digit3) {
                Some(HexDirection::Three)
            } else if keyboard_input.pressed(KeyCode::Digit4) {
                Some(HexDirection::Four)
            } else if keyboard_input.pressed(KeyCode::Digit5) {
                Some(HexDirection::Five)
            } else {
                None
            };

            if let Some(hex_direction) = selected_hex_direction {
                let tile_pos = match map_type {
                    TilemapType::Hexagon(hex_coord_sys) => {
                        // Get the neighbor in a particular direction.
                        // This function does not check to see if the calculated neighbor lies
                        // within the tile map.
                        hex_direction.offset(hovered_tile_pos, *hex_coord_sys)
                    }
                    _ => unreachable!(),
                };

                // We want to ensure that the tile position lies within the tile map, so we do a
                // `checked_get`.
                if let Some(tile_entity) = tile_storage.checked_get(&tile_pos) {
                    if let Ok(label) = tile_label_q.get(tile_entity) {
                        if let Ok(mut text_color) = text_q.get_mut(label.0) {
                            text_color.0 = palettes::tailwind::GREEN_600.into();
                            commands.entity(tile_entity).insert(NeighborHighlight);
                        }
                    }
                }
            }
        }
    }
}

#[derive(SystemSet, Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct SpawnTilemapSet;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: String::from(
                            "Hexagon Neighbors - Hover over a tile, and then press 0-5 to mark neighbors",
                        ),
                        ..Default::default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(TilemapPlugin)
        .init_resource::<CursorPos>()
        .init_resource::<TileHandleHexCol>()
        .init_resource::<TileHandleHexRow>()
        .add_systems(Startup, (spawn_tilemap, ApplyDeferred).chain().in_set(SpawnTilemapSet))
        .add_systems(Startup, (spawn_tile_labels, spawn_map_type_label).after(SpawnTilemapSet))
        .add_systems(First, (camera_movement, update_cursor_pos).chain())
        .add_systems(Update, swap_map_type)
        .add_systems(Update, hover_highlight_tile_label.after(swap_map_type))
        .add_systems(Update, highlight_neighbor_label.after(hover_highlight_tile_label))
        .run();
}
