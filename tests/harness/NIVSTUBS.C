/*
	NIVSTUBS.C - linker stubs for the test harness.

	The harness links the REAL, UNMODIFIED generation modules NOCTIS-0.OBJ and
	NOCTIS-1.OBJ, but deliberately does NOT link NOCTIS.OBJ (the game's main
	module, which is full of graphics/mouse/DOS-interactive code we neither
	want nor can run head-less).

	NOCTIS-0/1 reference a handful of symbols that happen to be *defined* in
	NOCTIS.CPP. Those become unresolved externals once NOCTIS.OBJ is omitted.
	Every one of them lives only in code paths the harness never executes
	(movie recording, on-board labels, the F1 "about" page), so providing
	inert definitions here is safe and does NOT alter generation behaviour.

	The exhaustive list was obtained by scanning every `extern` in
	NOCTIS-0.CPP / NOCTIS-1.CPP and resolving where each symbol is defined.
*/

/* data symbols defined in NOCTIS.CPP that N0/N1 reference */
char about = 0;
long star_label_pos   = -1;
long planet_label_pos = -1;
char star_label[25]   = "UNKNOWN STAR / CLASS ...";
char planet_label[25] = "NAMELESS PLANET / N. ...";

/* function defined in NOCTIS.CPP that N0/N1 reference (F1 help overlay). */
void ShowAboutPage(char surface)
{
	/* never called by the generation code paths under test */
	(void)surface;
}
