import playground;
import common;
import binding;

[format("rgba8")]
[playground::BLACK_SCREEN(1.0, 1.0)]
WTexture2D<float4> screen;

[playground::KEY("KeyW")]
uniform float wPressed;
[playground::KEY("KeyA")]
uniform float aPressed;
[playground::KEY("KeyS")]
uniform float sPressed;
[playground::KEY("KeyD")]
uniform float dPressed;
[playground::KEY("Space")]
uniform float spacePressed;
[playground::KEY("ControlLeft")]
uniform float ctrlPressed;
[playground::KEY("ShiftLeft")]
uniform float shiftPressed;

[playground::KEY("Digit1")]
uniform float createWater;

[playground::SLIDER(5.0, 0.0, 20.0)]
uniform float speed;
[playground::DELTA_TIME]
uniform float deltaTime;

[playground::CALL(1, 1, 1)]
[shader("compute")]
[numthreads(1, 1, 1)]
void update(uint2 dispatchThreadId: SV_DispatchThreadID)
{
    uint2 imageSize;
    screen.GetDimensions(imageSize.x, imageSize.y);

    float3 cameraPos = gameData[0].playerPos;

    float3 cameraDir = getCameraDirection(float2(imageSize));
    float3 up = float3(0.0, 1.0, 0.0);
    float3 left = normalize(cross(up, cameraDir));

    float3 inputVector = (1.0 + shiftPressed) * speed * deltaTime * float3(wPressed - sPressed, aPressed - dPressed, spacePressed - ctrlPressed);
    float3x3 inputTransform = float3x3(cameraDir, left, up);

    cameraPos += mul(inputVector, inputTransform);

    gameData[0].playerPos = cameraPos;

    if (createWater > 0)
    {
        set_data(int3(gameData[0].playerPos), MAT_WATER << 24);
    }

    gameData[0].voxelOffset = (gameData[0].voxelOffset + 1) % 8;

    let worldCenter = (gameData[0].startPos + RENDER_SIZE / 2);
    let cameraChunk = int3(floor(cameraPos / CHUNK_SIZE));
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