/* libunify — small helpers shared by the UnifyOS GTK4 applications.
 *
 * Every application is a thin, honest front end over the Rust backends:
 * `unify-ctl` for hardware and system state, `unify-pkg` for packages,
 * `unify-sysmond` for live samples and `unify-installer` for installation.
 * Nothing is mocked; when a backend is missing the application says so.
 */

#ifndef UNIFY_LIBUNIFY_H
#define UNIFY_LIBUNIFY_H

#include <gtk/gtk.h>

/* Run a program and return its standard output (NUL terminated, caller frees).
 * Returns NULL when the program cannot be executed. */
char *unify_run(const char *const argv[]);

/* Run a program and return TRUE when it exits with status 0. */
gboolean unify_run_ok(const char *const argv[]);

/* Path of a backend binary, honouring the UNIFY_BINDIR environment variable so
 * the suite can be tested from a build tree. Returns a newly allocated string. */
char *unify_tool(const char *name);

/* Read a top-level or nested `"key": "value"` string from a JSON document.
 * This is deliberately a scanner, not a parser: it is used on documents the
 * backends emit, where keys are unique. */
char *unify_json_string(const char *json, const char *key);

/* Same, for numbers; returns `fallback` when the key is absent. */
double unify_json_number(const char *json, const char *key, double fallback);

/* Same, for booleans. */
gboolean unify_json_bool(const char *json, const char *key, gboolean fallback);

/* Window with a title, a header bar and a default size. */
GtkWidget *unify_window(GtkApplication *app, const char *title, int width, int height);

/* Window whose body is a scrolled window around `child`. */
GtkWidget *unify_scrolled(GtkWidget *child);

/* A list row with a bold title and an optional dim subtitle. */
GtkWidget *unify_row(const char *title, const char *subtitle);

/* A section heading label. */
GtkWidget *unify_heading(const char *text);

/* A read-only monospace log view; append through unify_log_append(). */
GtkWidget *unify_log_view(void);
void unify_log_append(GtkWidget *scrolled, const char *text);

/* Load the UnifyOS stylesheet if it is installed, otherwise the built-in one. */
void unify_install_css(void);

#endif /* UNIFY_LIBUNIFY_H */
