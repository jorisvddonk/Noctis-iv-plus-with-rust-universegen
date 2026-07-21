#include "defs.h"
#include "noctis-d.h"
#include "noctis-0.h"
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
extern char _delay;
void main(int argc, char **argv) {
    int n;
    if (argc < 4) return;
    _delay = 0;
    ap_target_x = atof(argv[1]);
    // Input y is "user-facing Parsis" (negated in the galactic chart).
    // The engine/starmap stores |y|, so fabs() makes it engine-facing.
    ap_target_y = fabs(atof(argv[2]));
    ap_target_z = atof(argv[3]);
    extract_ap_target_infos();
    prepare_nearstar();
    printf("CLASS=%d NOP=%d NOB=%d\n", nearstar_class, nearstar_nop, nearstar_nob);
    for (n = 0; n < nearstar_nob; n++)
        printf("BODY %d TYPE=%d OWNER=%d\n", n, nearstar_p_type[n], nearstar_p_owner[n]);
}
