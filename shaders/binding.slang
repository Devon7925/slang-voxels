import playground;
import common;

[playground::TIME]
float time;
[playground::MOUSE_POSITION]
float4 mousePosition;
[playground::FRAME_ID]
uniform float frame_id;

static const int RENDER_SIZE_X = 32;
static const int RENDER_SIZE_Y = 16;
static const int RENDER_SIZE_Z = 32;
static const int MAX_CHUNKS = 2048;

static const int MAX_CHUNK_LOADS = 256;
static const int MAX_CHUNK_UPDATES = 512;

static const int RESERVED_CHUNKS = 2;
static const int MAX_DIMENSION = 2048;
static const int MAX_DIMENSION_CHUNKS = MAX_DIMENSION / CHUNK_SIZE;
static const int MAX_CHUNKS_X = MAX_CHUNKS < MAX_DIMENSION_CHUNKS ? MAX_CHUNKS : MAX_DIMENSION_CHUNKS;
static const int X_CHUNK_LAYERS = (MAX_CHUNKS + MAX_CHUNKS_X - 1) / MAX_CHUNKS_X;
static const int MAX_CHUNKS_Y = X_CHUNK_LAYERS < MAX_DIMENSION_CHUNKS ? X_CHUNK_LAYERS : MAX_DIMENSION_CHUNKS;
static const int XY_CHUNK_LAYERS = (MAX_CHUNKS + MAX_CHUNKS_X * MAX_CHUNKS_Y - 1) / MAX_CHUNKS_X / MAX_CHUNKS_Y;
static const int MAX_CHUNKS_Z = XY_CHUNK_LAYERS < MAX_DIMENSION_CHUNKS ? XY_CHUNK_LAYERS : MAX_DIMENSION_CHUNKS;

static const int3 RENDER_SIZE = int3(RENDER_SIZE_X, RENDER_SIZE_Y, RENDER_SIZE_Z);

[playground::BLACK_3D(RENDER_SIZE_X, RENDER_SIZE_Y, RENDER_SIZE_Z)]
RWTexture3D<uint> chunks;

[playground::BLACK_3D(CHUNK_SIZE * MAX_CHUNKS_X, CHUNK_SIZE *MAX_CHUNKS_Y, CHUNK_SIZE *MAX_CHUNKS_Z)]
RWTexture3D<uint> voxels;

struct AvailableChunkData {
    Atomic<int> chunksAvailableCount;
    Atomic<int> chunksToMakeAvailableCount;
    int[MAX_CHUNKS - RESERVED_CHUNKS] available_chunks;
    int[MAX_CHUNKS - RESERVED_CHUNKS] available_chunks_queue;
}

[playground::ZEROS(1)]
RWStructuredBuffer<AvailableChunkData> availableChunkData;

struct ChunkLoadRequest
{
    int3 chunk_pos;
    int chunk_ptr;

    __init(int3 set_chunk_pos, int set_chunk_ptr, bool is_forced) {
        chunk_pos = set_chunk_pos;
        chunk_ptr = set_chunk_ptr;
        if (is_forced)
        {
            chunk_ptr *= -1;
        }
    }

    int3 get_chunk_pos() {
        return chunk_pos;
    }

    int get_chunk_ptr() {
        return abs(chunk_ptr);
    }

    bool get_is_forced() {
        return chunk_ptr < 0;
    }
}

struct UpdateRequests {
    ChunkLoadRequest[MAX_CHUNK_LOADS] chunk_load_requests;
    ChunkLoadRequest[MAX_CHUNK_LOADS] active_chunk_load_requests;
    Array<int3[MAX_CHUNK_UPDATES], 8> chunk_update_requests;
    int3[MAX_CHUNK_UPDATES] active_chunk_update_requests;
}

[playground::ZEROS(1)]
RWStructuredBuffer<UpdateRequests> updateRequests;

struct PlayerInput
{
    float forward;
    float backward;
    float left;
    float right;
    float jump;
    float crouch;
    float interact;
    float mouseSensitivity;
}

[playground::EXTERNAL_UNIFORM]
uniform PlayerInput playerInput;

[playground_ANNOTATION("derive(EguiProbe)")]
struct GraphicsSettings
{
    uint primary_ray_dist;
    uint shadow_ray_dist;
    uint reflection_ray_dist;
    uint ao_ray_dist;
}

[playground::EXTERNAL_UNIFORM]
uniform GraphicsSettings graphicsSettings;

struct GameData
{
    Atomic<int>[8 * RENDER_SIZE_X * RENDER_SIZE_Y * RENDER_SIZE_Z] chunkUpdateRequestCounts;
    Atomic<int>[RENDER_SIZE_X * RENDER_SIZE_Y * RENDER_SIZE_Z] chunkLoadRequestCounts;
    float4 prevMouse;
    int voxelOffset;
    int3 startPos;
    Atomic<int> currentChunkLoads;
    Atomic<int>[8] currentChunkUpdates;
}

[playground::ZEROS(1)]
RWStructuredBuffer<GameData> gameData;

[playground::ZEROS(1)]
RWStructuredBuffer<Player> players;

[playground::ZEROS(20)]
RWStructuredBuffer<uint> indirectBuffer;

[playground::CALL(RENDER_SIZE_X * RENDER_SIZE_Y * RENDER_SIZE_Z, 1, 1)]
[playground::CALL_ONCE]
[shader("compute")]
[numthreads(16, 1, 1)]
void clearInitialUpdateCounts(uint3 dispatchThreadId: SV_DispatchThreadID)
{
    if (dispatchThreadId.x >= RENDER_SIZE_X * RENDER_SIZE_Y * RENDER_SIZE_Z)
        return;
    for (var i = 0; i < 8; i++)
    {
        gameData[0].chunkUpdateRequestCounts[dispatchThreadId.x + i * RENDER_SIZE_X * RENDER_SIZE_Y * RENDER_SIZE_Z].store(0);
    }
    gameData[0].chunkLoadRequestCounts[dispatchThreadId.x].store(0);
}

[playground::CALL(MAX_CHUNK_UPDATES, 1, 1)]
[playground::CALL_ONCE]
[shader("compute")]
[numthreads(16, 1, 1)]
void clearInitialUpdates(uint3 dispatchThreadId: SV_DispatchThreadID)
{
    if (dispatchThreadId.x >= MAX_CHUNK_UPDATES)
        return;
    for (var i = 0; i < 8; i++)
    {
        updateRequests[0].chunk_update_requests[i][dispatchThreadId.x] = int3(0, 0, 0);
    }
}

[playground::CALL(MAX_CHUNKS, 1, 1)]
[playground::CALL_ONCE]
[shader("compute")]
[numthreads(16, 1, 1)]
void init_available(uint3 dispatchThreadId: SV_DispatchThreadID)
{
    if (dispatchThreadId.x >= MAX_CHUNKS - RESERVED_CHUNKS)
        return;
    availableChunkData[0].available_chunks[dispatchThreadId.x] = dispatchThreadId.x + RESERVED_CHUNKS;
}

int3 get_chunk_pos_of_global_pos(int3 global_pos)
{
    return floorDiv(global_pos, CHUNK_SIZE);
}

uint3 get_chunk_ptr_of_chunk_pos(int3 chunk_pos)
{
    return floorMod(chunk_pos, RENDER_SIZE);
}

uint get_update_index_of_chunk_ptr(uint update_type, uint3 chunk_ptr)
{
    return update_type * RENDER_SIZE_X * RENDER_SIZE_Y * RENDER_SIZE_Z + chunk_ptr.x + chunk_ptr.y * RENDER_SIZE_X + chunk_ptr.z * RENDER_SIZE_X * RENDER_SIZE_Y;
}

uint get_update_index_of_chunk_pos(uint update_type, int3 chunk_pos)
{
    return get_update_index_of_chunk_ptr(update_type, get_chunk_ptr_of_chunk_pos(chunk_pos));
}

uint get_chunk_voxels_index_of_chunk_pos(int3 chunk_pos)
{
    return chunks.Load(get_chunk_ptr_of_chunk_pos(chunk_pos));
}

uint3 get_chunk_ptr_of_chunk_index(uint chunk_idx)
{
    return int3(chunk_idx % MAX_CHUNKS_X, (chunk_idx / MAX_CHUNKS_X) % MAX_CHUNKS_Y, chunk_idx / MAX_CHUNKS_X / MAX_CHUNKS_Y);
}

uint get_chunk_voxels_index_of_global_pos(int3 global_pos)
{
    return get_chunk_voxels_index_of_chunk_pos(get_chunk_pos_of_global_pos(global_pos));
}

int3 get_voxel_ptr_of_global_pos(int3 global_pos)
{
    let chunk_pos = get_chunk_pos_of_global_pos(global_pos);
    let chunk_idx = get_chunk_voxels_index_of_chunk_pos(chunk_pos);
    let chunk_loc = get_chunk_ptr_of_chunk_index(chunk_idx);
    return CHUNK_SIZE * chunk_loc + floorMod(global_pos, CHUNK_SIZE);
}

void unload_chunk(int3 chunk_pos)
{
    let chunk_ptr = get_chunk_ptr_of_chunk_pos(chunk_pos);
    let voxel_data_idx = chunks.Load(chunk_ptr);
    if (voxel_data_idx != UNLOADED_CHUNK)
    {
        chunks.Store(chunk_ptr, UNLOADED_CHUNK);
    }
    if (voxel_data_idx < RESERVED_CHUNKS)
        return;
    free_chunk(voxel_data_idx);
}

bool chunk_in_bounds(int3 chunk_pos)
{
    if (any(chunk_pos < gameData[0].startPos))
        return false;
    if (any(chunk_pos >= gameData[0].startPos + RENDER_SIZE))
        return false;

    return true;
}

void load_chunk(int3 chunk_pos, bool is_forced)
{
    if (!chunk_in_bounds(chunk_pos))
        return;
    let checkChunkIdx = floorMod(chunk_pos, RENDER_SIZE);
    let chunk_idx = checkChunkIdx.x + checkChunkIdx.y * RENDER_SIZE_X + checkChunkIdx.z * RENDER_SIZE_X * RENDER_SIZE_Y;
    let was_already_queued = gameData[0].chunkLoadRequestCounts[chunk_idx].increment();
    if (was_already_queued >= 1)
    {
        gameData[0].chunkLoadRequestCounts[chunk_idx].decrement();
        return;
    }
    let availableChunkIdx = availableChunkData[0].chunksAvailableCount.decrement() - 1;
    if (availableChunkIdx < 0)
    {
        gameData[0].chunkLoadRequestCounts[chunk_idx].decrement();
        availableChunkData[0].chunksAvailableCount.increment();
        return;
    }
    let chunk = availableChunkData[0].available_chunks[availableChunkIdx];
    let chunkLoadIdx = gameData[0].currentChunkLoads.increment();
    if (chunkLoadIdx >= MAX_CHUNK_LOADS)
    {
        gameData[0].chunkLoadRequestCounts[chunk_idx].decrement();
        gameData[0].currentChunkLoads.decrement();
        free_chunk(chunk);
        return;
    }
    updateRequests[0].chunk_load_requests[chunkLoadIdx] = ChunkLoadRequest(chunk_pos, chunk, is_forced);
}

void free_chunk(int chunk) {
    let idx = availableChunkData[0].chunksToMakeAvailableCount.increment();
    availableChunkData[0].available_chunks_queue[idx] = chunk;
}

void queue_update_all(int3 chunk_pos)
{
    if (!chunk_in_bounds(chunk_pos))
        return;
    let chunk_ptr = get_chunk_ptr_of_chunk_pos(chunk_pos);
    let voxel_data_ptr = chunks.Load(chunk_ptr);
    if (voxel_data_ptr == UNLOADED_CHUNK)
    {
        load_chunk(chunk_pos, false);
        return;
    }
    if (voxel_data_ptr < RESERVED_CHUNKS)
    {
        return;
    }
    for (var chunk_update_type = 0; chunk_update_type < 8; chunk_update_type++)
    {
        let update_idx = get_update_index_of_chunk_ptr(chunk_update_type, chunk_ptr);
        let prev_chunk_queue_request_count = gameData[0].chunkUpdateRequestCounts[update_idx].increment();
        if (prev_chunk_queue_request_count > 0)
        {
            gameData[0].chunkUpdateRequestCounts[update_idx].decrement();
            continue;
        }
        let typed_update_idx = gameData[0].currentChunkUpdates[chunk_update_type].increment();
        if (typed_update_idx >= MAX_CHUNK_UPDATES)
        {
            gameData[0].chunkUpdateRequestCounts[update_idx].decrement();
            gameData[0].currentChunkUpdates[chunk_update_type].decrement();
            continue;
        }
        updateRequests[0].chunk_update_requests[chunk_update_type][typed_update_idx] = chunk_pos;
    }
}

[playground::CALL_ONCE]
[playground::CALL(1, 1, 1)]
[shader("compute")]
[numthreads(1, 1, 1)]
void setup(uint3 dispatchThreadId: SV_DispatchThreadID)
{
    players[0] = Player::default();

    uint3 voxelSize;
    voxels.GetDimensions(voxelSize.x, voxelSize.y, voxelSize.z);
    indirectBuffer[0] = 0;
    indirectBuffer[1] = CHUNK_SIZE / VOXEL_UPDATE_SIZE / 2;
    indirectBuffer[2] = CHUNK_SIZE / VOXEL_UPDATE_SIZE / 2;

    indirectBuffer[4] = 0;
    indirectBuffer[5] = CHUNK_SIZE / 4;
    indirectBuffer[6] = CHUNK_SIZE / 4;

    indirectBuffer[8] = 0;
    indirectBuffer[9] = 1;
    indirectBuffer[10] = 1;

    indirectBuffer[12] = 0;
    indirectBuffer[13] = 1;
    indirectBuffer[14] = 1;

    gameData[0].startPos = int3(0);
    availableChunkData[0].chunksAvailableCount.store(MAX_CHUNKS - RESERVED_CHUNKS);
    availableChunkData[0].chunksToMakeAvailableCount.store(0);
    gameData[0].currentChunkLoads.store(0);
    for (var i = 0; i < 8; i++)
    {
        gameData[0].currentChunkUpdates[i].store(0);
    }
    gameData[0].voxelOffset = 0;
}

[playground::CALL_ONCE]
[playground::CALL(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE)]
[shader("compute")]
[numthreads(4, 4, 4)]
void setReservedChunks(uint3 dispatchThreadId: SV_DispatchThreadID)
{
    voxels.Store(dispatchThreadId, MAT_OOB << 24);
    voxels.Store(dispatchThreadId + uint3(CHUNK_SIZE, 0, 0), MAT_AIR_OOB << 24);
}

int3 get_voxel_update_offset()
{
    let idx = int(gameData[0].voxelOffset);
    return int3(idx & 1, -((idx >> 1) & 1), (idx >> 2) & 1);
}

uint get_data_unchecked(int3 global_pos)
{
    return voxels.Load(get_voxel_ptr_of_global_pos(global_pos));
}

uint get_data(int3 global_pos)
{
    int3 start_offset = CHUNK_SIZE * gameData[0].startPos;
    if (any(global_pos < start_offset))
        return MAT_OOB << 24;
    int3 rel_pos = global_pos - start_offset;
    if (any(rel_pos >= CHUNK_SIZE * RENDER_SIZE))
        return MAT_OOB << 24;
    return get_data_unchecked(global_pos);
}

void set_data(int3 global_pos, uint data)
{
    uint og_voxel_data = get_data(global_pos);
    if (og_voxel_data >> 24 == MAT_OOB)
        return;
    if (og_voxel_data >> 24 == MAT_AIR_OOB) {
        if (data >> 24 == MAT_AIR || data >> 24 == MAT_AIR_OOB) {
            return;
        }
        load_chunk(get_chunk_pos_of_global_pos(global_pos), true);
        return;
    }
    if (og_voxel_data != data)
    {
        let chunk_pos = get_chunk_pos_of_global_pos(global_pos);
        let pos_in_chunk = floorMod(global_pos, CHUNK_SIZE);
        let min_pos = select(pos_in_chunk == 0, -1, 0);
        let max_pos = select(pos_in_chunk == CHUNK_SIZE - 1, 1, 0);
        for (var dx = min_pos.x; dx <= max_pos.x; dx++)
            for (var dy = min_pos.y; dy <= max_pos.y; dy++)
                for (var dz = min_pos.z; dz <= max_pos.z; dz++)
                    queue_update_all(chunk_pos + int3(dx, dy, dz));
    }
    voxels.Store(get_voxel_ptr_of_global_pos(global_pos), data);
}
