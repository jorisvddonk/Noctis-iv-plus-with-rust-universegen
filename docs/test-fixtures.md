# Planet Type Test Fixtures

BCPP 3.1 reference output (`planet-all -x -y -z -z` via DOSBox-X).

## Unnamed class 1 star (80 bodies)

Our primary test system. Produces all 10 planet types except type 3.

```
-x 3612345 -y -4487654 -z -858973
```

Types: 7(19) 5(16) 1(14) 2(8) 8(6) 4(6) 6(5) 9(3) 0(3)

| Type | Body | Description |
|------|------|-------------|
| 0 | 23 | medium size, internally hot, unstable surface, no atmosphere. |
| 1 | 3 | small, solid, dusty, craterized, no atmosphere. |
| 2 | 13 | medium size, solid, thick atmosphere, fully covered by clouds. |
| 4 | 2 | medium size, rocky, creased, no atmosphere. |
| 5 | 1 | small, solid, thin atmosphere. |
| 6 | 6 | large, not consistent, covered with dense clouds. |
| 7 | 9 | small, solid, icy surface, no atmosphere. |
| 8 | 0 | medium size, surface is mainly native quartz, oxygen atmosphere. |
| 9 | 7 | very large, substellar object, not consistent. |
| 10 | — | companion star (surface() is a no-op) |

## Balastrackonastreya — class 0 (22 bodies)

```
-x -18928 -y -29680 -z -67336
```

Types: 1(6) 5(3) 3(3) 2(3) 8(2) 4(2) 0(2) 6(1)

| Type | Body | Description |
|------|------|-------------|
| 3 | 3 | medium size, felisian, breathable atmosphere, suitable for life. |

## FENIA — class 0 (19 bodies)

```
-x 4342128 -y -4559934 -z -807862
```

Types: 1(11) 3(3) 8(1) 6(1) 2(1) 5(1)

| Type | Body | Description |
|------|------|-------------|
| 3 | 2 | medium size, felisian, breathable atmosphere, suitable for life. |

(Other types available — run `planet-all` to get full listing.)

## Additional star systems

Classes 5 (brown dwarf) and 6 (dead star) have no planets by game design.
All coordinates from `noctis-starmap` starmap2.bin. Negate Y for Noctis
internal coordinates (game flips Y).

| Class | Star | Bodies | Type counts | Coords (-x, -y, -z) |
|-------|------|--------|-------------|----------------------|
| 0 | INIX I | 28 | 1(11) 3(4) 8(3) 5(3) 4(2) 2(2) 9(1) 6(1) 0(1) | -x -1998254512 -y 9508 -z 2380852 |
| 1 | GRAVIL II | 23 | 1(11) 5(3) 0(3) 7(2) 4(2) 9(1) 2(1) | -x -1998888432 -y 1435 -z 873557 |
| 2 | KASZEL | 6 | 6(4) 8(2) | -x -107404880 -y -24973547 -z -1984549 |
| 3 | SLINRE | 26 | 2(5) 1(5) 5(4) 8(3) 3(3) 6(2) 4(2) 9(1) 0(1) | -x -1997637008 -y 9992 -z 759944 |
| 4 | SADA'UKEIRIT | 80 | 7(33) 1(12) 9(9) 5(7) 2(5) 8(4) 0(4) 6(3) 4(2) 3(1) | -x -1218798544 -y 1041409 -z -1081208337 |
| 5 | BROWN GAS | 0 | — | -x -653187152 -y 321344 -z 72 |
| 6 | L4 FINAL STOP | 0 | — | -x -1998519504 -y 24432 -z 641552 |
| 7 | BLUEK | 1 | 9(1) | -x -1994744880 -y -121633 -z 1270993 |
| 8 | TRIVIAPOLEN | 4 | 10(2) 6(1) 1(1) | -x -1997774864 -y 4221 -z 649715 |
| 9 | YOLINX I | 72 | 1(17) 7(14) 5(12) 2(10) 9(4) 4(4) 8(3) 6(3) 0(3) 3(2) | -x -1998481648 -y 3234 -z 934018 |
| 10 | DOTURAK | 1 | 2(1) | -x -822289168 -y 1295379 -z -679183759 |
| 11 | WHIKEL I | 5 | 1(3) 5(2) | -x -1996240944 -y -72703 -z 944799 |
