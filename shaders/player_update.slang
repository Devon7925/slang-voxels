import playground;
import common;
import binding;

[playground::KEY("Digit1")]
uniform float createWater;
[playground::DELTA_TIME]
uniform float deltaTime;

// A small buffer to prevent the player from getting stuck in geometry due to floating-point errors.
static const float SKIN_WIDTH = 0.005f;
// The maximum height the player can automatically step up.
static const float MAX_STEP_HEIGHT = 1.0f;

// Helper: Intersects a ray with a static AABB.
// Returns true and updates 't' and 'normal' if a valid collision is found within the time interval [0, t].
bool ray_vs_aabb(
    float3 ray_origin,
    float3 ray_inv_dir, // Pre-calculated 1.0 / ray_direction
    float3 aabb_min,
    float3 aabb_max,
    inout float t,
    out float3 normal)
{
    float3 t_min_vec = (aabb_min - ray_origin) * ray_inv_dir;
    float3 t_max_vec = (aabb_max - ray_origin) * ray_inv_dir;

    float3 t_near_vec = min(t_min_vec, t_max_vec);
    float3 t_far_vec = max(t_min_vec, t_max_vec);

    float t_near = max(max(t_near_vec.x, t_near_vec.y), t_near_vec.z);
    float t_far = min(min(t_far_vec.x, t_far_vec.y), t_far_vec.z);

    if (t_near >= t_far || t_far <= 0.0 || t_near >= t)
    {
        return false;
    }

    t = t_near;

    if (t_near_vec.x > t_near_vec.y && t_near_vec.x > t_near_vec.z) {
        normal = float3(-sign(ray_inv_dir.x), 0, 0);
    } else if (t_near_vec.y > t_near_vec.x && t_near_vec.y > t_near_vec.z) {
        normal = float3(0, -sign(ray_inv_dir.y), 0);
    } else {
        normal = float3(0, 0, -sign(ray_inv_dir.z));
    }

    return true;
}

// Sweeps the player's AABB against the world to find the first time of collision.
bool sweep_test(
    Player player,
    float3 move_vec,
    inout float collision_time, // in: max time (1.0), out: actual time
    out float3 collision_normal,
    out uint hit_material)
{
    collision_normal = float3(0, 0, 0);
    hit_material = 0;
    bool found_collision = false;

    if (dot(move_vec, move_vec) < 1e-9) return false;

    float3 player_center = player.pos.xyz + player.size * PLAYER_HITBOX_OFFSET;
    float3 player_half_size = player.size * PLAYER_HITBOX_SIZE * 0.5;

    float3 sweep_min = min(player_center - player_half_size, player_center - player_half_size + move_vec);
    float3 sweep_max = max(player_center + player_half_size, player_center + player_half_size + move_vec);
    int3 min_voxel = int3(floor(sweep_min));
    int3 max_voxel = int3(floor(sweep_max));

    float3 inv_move_vec = 1.0f / (move_vec + 1e-9f * sign(move_vec)); // Add epsilon to avoid div by zero

    for (int x = min_voxel.x; x <= max_voxel.x; ++x)
        for (int y = min_voxel.y; y <= max_voxel.y; ++y)
            for (int z = min_voxel.z; z <= max_voxel.z; ++z)
            {
                uint material_id = get_data(int3(x, y, z)) >> 24;
                if (physics_properties[material_id].is_fluid) continue;

                float3 expanded_voxel_min = float3(int3(x, y, z)) - player_half_size;
                float3 expanded_voxel_max = float3(int3(x, y, z)) + float3(1, 1, 1) + player_half_size;

                float current_t = collision_time;
                float3 current_normal;

                if (ray_vs_aabb(player_center, inv_move_vec, expanded_voxel_min, expanded_voxel_max, current_t, current_normal))
                {
                    found_collision = true;
                    collision_time = current_t;
                    collision_normal = current_normal;
                    hit_material = material_id;
                }
            }

    return found_collision;
}

// Helper that checks if a given AABB is free of solid voxels.
bool is_box_clear(float3 box_min, float3 box_max)
{
    int3 min_voxel = int3(floor(box_min));
    int3 max_voxel = int3(floor(box_max));

    for (int x = min_voxel.x; x <= max_voxel.x; ++x)
        for (int y = min_voxel.y; y <= max_voxel.y; ++y)
            for (int z = min_voxel.z; z <= max_voxel.z; ++z)
            {
                uint material_id = get_data(int3(x, y, z)) >> 24;
                if (!physics_properties[material_id].is_fluid) {
                    return false;
                }
            }
    return true;
}

bool attempt_step_up(inout Player player, float3 move_dir)
{
    // Use a normalized, short-distance vector for checks to ensure they are local to the player.
    float3 check_dir = normalize(float3(move_dir.x, 0, move_dir.z));
    float check_dist = player.size.x * PLAYER_HITBOX_SIZE.x * 0.5f + SKIN_WIDTH;
    float3 horizontal_check_offset = check_dir * check_dist;

    float3 player_center = player.pos.xyz + player.size * PLAYER_HITBOX_OFFSET;
    float3 player_half_size = player.size * PLAYER_HITBOX_SIZE * 0.5;
    float3 player_min = player_center - player_half_size;

    // 1. Check headroom: Ensure space directly above the player is clear to avoid phasing through ceilings.
    float3 headroom_min = player_min + float3(0, player_half_size.y * 2, 0);
    float3 headroom_max = headroom_min + float3(player_half_size.x * 2, MAX_STEP_HEIGHT, player_half_size.z * 2);
    if (!is_box_clear(headroom_min, headroom_max))
    {
        return false;
    }

    // 2. Check path clearance: Ensure the volume the player will move into during the step is clear.
    float3 step_path_min = player_min + horizontal_check_offset;
    step_path_min.y += MAX_STEP_HEIGHT;
    float3 step_path_max = step_path_min + player_half_size * 2;
    if (!is_box_clear(step_path_min, step_path_max))
    {
        return false;
    }

    // 3. Ledge check: Ensure there is a solid surface to land on. THIS IS THE CORRECTED LOGIC.
    // We define a thin, wide box at the destination to check for ground.
    float3 ledge_box_max = player_min + horizontal_check_offset + float3(player_half_size.x * 2, 0, player_half_size.z * 2);
    ledge_box_max.y += MAX_STEP_HEIGHT;
    float3 ledge_box_min = ledge_box_max - float3(player_half_size.x * 2, 0.2f, player_half_size.z * 2);

    // The AABB is now correctly defined (min < max).
    if (is_box_clear(ledge_box_min, ledge_box_max))
    {
        return false; // No ground to stand on.
    }

    // All checks passed. Perform the step.
    player.pos.y += MAX_STEP_HEIGHT;
    player.vel.y = max(0.0, player.vel.y); // Prevent losing jump velocity
    return true;
}

void apply_sliding_friction(inout float3 vel, float3 normal, float friction_factor, float deltaTime)
{
    float3 parallel_vel = vel - normal * dot(vel, normal);
    float parallel_speed = length(parallel_vel);

    if (parallel_speed > 1e-6)
    {
        float drop = (friction_factor * 0.5 + friction_factor * parallel_speed) * deltaTime;
        float new_speed = max(0.0, parallel_speed - drop);
        parallel_vel *= (new_speed / parallel_speed);
        vel = parallel_vel + normal * dot(vel, normal);
    }
}

// Main collision function, using iterative swept AABB with skin width and robust step-up.
void collide_player(inout Player player, int3 prev_collision_vec)
{
    float3 move_total = player.vel * deltaTime;
    player.collision_vec = int3(0);

    const int MAX_SLIDES = 4;
    for (int i = 0; i < MAX_SLIDES; ++i)
    {
        if (dot(move_total, move_total) < 1e-9) break;

        float collision_time = 1.0;
        float3 collision_normal;
        uint hit_material;

        if (sweep_test(player, move_total, collision_time, collision_normal, hit_material))
        {
            // --- Step-Up Logic ---
            bool is_horizontal_hit = abs(collision_normal.y) < 0.1;
            bool was_on_ground = prev_collision_vec.y == 1;

            if (is_horizontal_hit && was_on_ground) // Only attempt step on first impact while grounded
            {
                if (attempt_step_up(player, float3(move_total.x, 0, move_total.z)))
                {
                    // Step was successful. We have been teleported UP.
                    // Now, we simply continue the loop to re-evaluate the movement
                    // from this new height. The original horizontal velocity is preserved.
                    continue;
                }
            }

            // --- Standard Collision & Sliding ---
            // Move player to point of impact, leaving a tiny gap (skin width) to prevent getting stuck.
            player.pos.xyz += move_total * max(0.0, collision_time - SKIN_WIDTH / length(move_total));

            player.collision_vec = int3(round(collision_normal));

            float friction = physics_properties[hit_material].friction;
            apply_sliding_friction(player.vel, collision_normal, friction, deltaTime);

            float remaining_time = 1.0f - collision_time;
            move_total *= remaining_time;

            // Project remaining movement onto the collision plane to slide.
            move_total -= collision_normal * dot(move_total, collision_normal);

            // Stop velocity component that is moving into the wall.
            float vel_dot_normal = dot(player.vel, collision_normal);
            if (vel_dot_normal < 0) {
                player.vel -= collision_normal * vel_dot_normal;
            }
        }
        else // No collision found in the sweep.
        {
            player.pos.xyz += move_total;
            break;
        }
    }
}

[playground::CALL::SIZE_OF("players")]
[shader("compute")]
[numthreads(1, 1, 1)]
void update(uint2 dispatchThreadId: SV_DispatchThreadID)
{
    uint entity_idx = dispatchThreadId.x;
    // if (entity_idx >= sim_data.player_count) return;
    Player player = players[entity_idx];

    { // Handle player rotation based on mouse movement
        float2 mouse_delta = mousePosition.xy - gameData[0].prevMouse.xy;

        // Yaw rotation (left/right)
        float yaw_angle = -mouse_delta.x * playerInput.mouseSensitivity;
        float cos_yaw = cos(yaw_angle);
        float sin_yaw = sin(yaw_angle);
        float3x3 yaw_matrix = float3x3(
            cos_yaw, 0, sin_yaw,
            0, 1, 0,
            -sin_yaw, 0, cos_yaw
        );
        player.dir.xyz = mul(yaw_matrix, player.dir.xyz);

        // Pitch rotation (up/down)
        float pitch_angle = -mouse_delta.y * playerInput.mouseSensitivity;
        float3 right_vec = normalize(cross(player.dir.xyz, float3(0, 1, 0)));
        float cos_pitch = cos(pitch_angle);
        float sin_pitch = sin(pitch_angle);
        float3x3 pitch_matrix = float3x3(
            cos_pitch + right_vec.x * right_vec.x * (1 - cos_pitch), right_vec.x * right_vec.y * (1 - cos_pitch) - right_vec.z * sin_pitch, right_vec.x * right_vec.z * (1 - cos_pitch) + right_vec.y * sin_pitch,
            right_vec.y * right_vec.x * (1 - cos_pitch) + right_vec.z * sin_pitch, cos_pitch + right_vec.y * right_vec.y * (1 - cos_pitch), right_vec.y * right_vec.z * (1 - cos_pitch) - right_vec.x * sin_pitch,
            right_vec.z * right_vec.x * (1 - cos_pitch) - right_vec.y * sin_pitch, right_vec.z * right_vec.y * (1 - cos_pitch) + right_vec.x * sin_pitch, cos_pitch + right_vec.z * right_vec.z * (1 - cos_pitch)
        );

        float3 new_dir = mul(pitch_matrix, player.dir.xyz);

        // Clamp pitch to prevent looking straight up or down
        if (abs(new_dir.y) < 0.99)
        {
            player.dir.xyz = new_dir;
        }
        player.right.xyz = normalize(cross(player.dir.xyz, float3(0.0, 1.0, 0.0)));
    }
    gameData[0].prevMouse = mousePosition;

    float3 move_vec = float3(0.0);
    float3 player_forward = player.dir.xyz;
    float3 player_right = player.right.xyz;
    float speed_multiplier = 1.0;

    move_vec += player_forward * (playerInput.forward - playerInput.backward);
    move_vec += player_right * (playerInput.right - playerInput.left);
    move_vec += float3(0.0, 1.0, 0.0) * (0.25 * playerInput.jump - 0.6 * playerInput.crouch);
    speed_multiplier /= 1.0 + playerInput.crouch;

    if (length(move_vec) > 0.0)
        move_vec = normalize(move_vec);

    player.movement_direction = move_vec;

    // If collision_vec is not zero, use 80.0, else 18.0
    float accel_speed = speed_multiplier * (any(player.collision_vec != int3(0)) ? 80.0 : 18.0);

    static const float SPEED = 1.0;
    float3 speed_vec = float3(SPEED, 1.0, SPEED);
    player.vel += accel_speed * (speed_vec * move_vec) * deltaTime;

    if (playerInput.jump > 0.0)
    {
        player.vel += speed_vec * player.collision_vec * float3(0.3, 13.0, 0.3);
    }

    // volume effects
    float3 start_pos =
        player.pos.xyz + player.size * PLAYER_HITBOX_OFFSET - player.size * PLAYER_HITBOX_SIZE / 2.0;
    float3 end_pos =
        player.pos.xyz + player.size * PLAYER_HITBOX_OFFSET + player.size * PLAYER_HITBOX_SIZE / 2.0;
    int3 start_voxel_pos = int3(floor(start_pos));
    int3 end_voxel_pos = int3(floor(end_pos));
    int3 iter_counts = end_voxel_pos - start_voxel_pos + int3(1);
    float nearby_density = 0.0;
    float3 directional_density = float3(0.0);
    for (int x = 0; x < iter_counts.x; x++)
    {
        for (int y = 0; y < iter_counts.y; y++)
        {
            for (int z = 0; z < iter_counts.z; z++)
            {
                int3 voxel_pos = start_voxel_pos + int3(x, y, z);
                float3 overlapping_dimensions = min(end_pos, float3(voxel_pos) + float3(1.0)) - max(start_pos, float3(voxel_pos));
                float overlapping_volume =
                    overlapping_dimensions.x * overlapping_dimensions.y * overlapping_dimensions.z;
                uint voxel = get_data(voxel_pos);
                float density = physics_properties[voxel >> 24].density;
                nearby_density += overlapping_volume * density;
                directional_density += overlapping_volume * density * ((float3(voxel_pos) + float3(0.5)) - (player.pos.xyz + player.size * PLAYER_HITBOX_OFFSET)) / player.size;
            }
        }
    }
    nearby_density /=
        player.size * player.size * player.size * PLAYER_HITBOX_SIZE.x * PLAYER_HITBOX_SIZE.y * PLAYER_HITBOX_SIZE.z;
    directional_density /=
        player.size * player.size * player.size * PLAYER_HITBOX_SIZE.x * PLAYER_HITBOX_SIZE.y * PLAYER_HITBOX_SIZE.z;

    if (player.has_world_collision == 1)
    {
        player.vel.xyz += (PLAYER_DENSITY - nearby_density) * player.gravity * 11.428571428571429 * deltaTime;
        if (length(directional_density) * deltaTime > 0.001)
        {
            player.vel.xyz -= 0.5 * directional_density * deltaTime;
        }
        if (dot(player.vel.xyz, player.vel.xyz) > 0.0)
        {
            player.vel.xyz -= nearby_density * 0.0375 * player.vel.xyz * length(player.vel.xyz) * deltaTime + 0.2 * normalize(player.vel.xyz) * deltaTime;
        }
        int3 prev_collision_vec = player.collision_vec;
        player.collision_vec = int3(0);
        collide_player(
            player,
            prev_collision_vec);
    }
    else
    {
        player.pos += player.vel;
        player.vel *= 0.5;
    }

    players[entity_idx] = player;

    if (createWater > 0)
    {
        set_data(int3(floor(player.pos)), MAT_WATER << 24);
    }

    gameData[0].voxelOffset = (gameData[0].voxelOffset + 1) % 8;

    let worldCenter = (gameData[0].startPos + RENDER_SIZE / 2);
    let cameraChunk = int3(floor(player.pos / CHUNK_SIZE));
    let playerOffset = (cameraChunk - worldCenter);
    let shift = (abs(playerOffset) >= 2) * sign(playerOffset);

    // Unload chunks at edges when shifting world
    if (any(shift != 0))
    {
        // Unload chunks at the opposite edge of movement
        for (int y = 0; y < RENDER_SIZE.y; y++)
        {
            for (int z = 0; z < RENDER_SIZE.z; z++)
            {
                if (shift.x > 0)
                {
                    let chunk_pos = gameData[0].startPos + int3(0, y, z);
                    unload_chunk(chunk_pos);
                }
                else if (shift.x < 0)
                {
                    let chunk_pos = gameData[0].startPos + int3(RENDER_SIZE.x - 1, y, z);
                    unload_chunk(chunk_pos);
                }
            }
        }

        for (int x = 0; x < RENDER_SIZE.x; x++)
        {
            for (int z = 0; z < RENDER_SIZE.z; z++)
            {
                if (shift.y > 0)
                {
                    let chunk_pos = gameData[0].startPos + int3(x, 0, z);
                    unload_chunk(chunk_pos);
                }
                else if (shift.y < 0)
                {
                    let chunk_pos = gameData[0].startPos + int3(x, RENDER_SIZE.y - 1, z);
                    unload_chunk(chunk_pos);
                }
            }
        }

        for (int x = 0; x < RENDER_SIZE.x; x++)
        {
            for (int y = 0; y < RENDER_SIZE.y; y++)
            {
                if (shift.z > 0)
                {
                    let chunk_pos = gameData[0].startPos + int3(x, y, 0);
                    unload_chunk(chunk_pos);
                }
                else if (shift.z < 0)
                {
                    let chunk_pos = gameData[0].startPos + int3(x, y, RENDER_SIZE.z - 1);
                    unload_chunk(chunk_pos);
                }
            }
        }
    }

    gameData[0].startPos += shift;

    for (int x = -2; x <= 2; x++)
    {
        for (int y = -2; y <= 2; y++)
        {
            for (int z = -2; z <= 2; z++)
            {
                let checkChunk = cameraChunk + int3(x, y, z);
                let checkChunkIdx = floorMod(checkChunk, RENDER_SIZE);
                if (chunks.Load(checkChunkIdx) == 0)
                {
                    load_chunk(checkChunk);
                }
            }
        }
    }
    AllMemoryBarrier();
    indirectBuffer[0] = CHUNK_SIZE / VOXEL_UPDATE_SIZE / 2 * gameData[0].currentChunkUpdates[gameData[0].voxelOffset].load();
    indirectBuffer[4] = CHUNK_SIZE / 4 * gameData[0].currentChunkLoads.load();
    indirectBuffer[8] = gameData[0].currentChunkLoads.load();

    gameData[0].currentChunkUpdates[gameData[0].voxelOffset].store(0);
    gameData[0].currentChunkLoads.store(0);
}
