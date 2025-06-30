import playground;
import common;
import binding;
import worldgen;
import render;
import voxel_update;
import player_update;

float4 imageMain(uint2 dispatchThreadID, int2 screenSize)
{
    let uv = (2.0 * (float2(dispatchThreadID) / float2(screenSize)) - 1.0) * float2(float(screenSize.x) / float(screenSize.y), 1.0);
    let cameraDir = getCameraDirection(screenSize);
    let up = float3(0.0, 1.0, 0.0);
    let cameraLeft = -normalize(cross(up, cameraDir));
    let cameraUp = cross(cameraLeft, cameraDir);

    let ray = normalize(cameraDir + uv.x * cameraLeft + uv.y * cameraUp);

    var pos = gameData[0].playerPos;

    RaycastResult primary_ray = raycast(pos, ray, primary_ray_dist);

    if (primary_ray.layer_count == 0)
    {
        return float4(1.0, 0.0, 0.0, 1.0);
    }

    return float4(get_color(pos, ray, primary_ray, screenSize.y), 1.0);
}
