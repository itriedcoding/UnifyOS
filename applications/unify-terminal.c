/* Unify Terminal — opens the configured terminal emulator, and falls back to a
 * command runner on systems where none is installed.
 *
 * UnifyOS ships `foot` as the terminal emulator. This application is the
 * launcher for it; the runner below is a real (if minimal) shell front end, not
 * a mock, and it says plainly that it is not a full terminal. */

#include "libunify.h"

#include <stdlib.h>
#include <string.h>

typedef struct {
    GtkWidget *output;
    GtkWidget *entry;
    GtkWidget *status;
} Terminal;

static void append(Terminal *terminal, const char *text)
{
    unify_log_append(terminal->output, text);
    unify_log_append(terminal->output, "\n");
}

static void on_run(GtkButton *button, gpointer data)
{
    (void)button;
    Terminal *terminal = data;
    const char *command = gtk_editable_get_text(GTK_EDITABLE(terminal->entry));
    if (command == NULL || *command == '\0')
        return;

    gchar *prompt = g_strdup_printf("$ %s\n", command);
    append(terminal, prompt);
    g_free(prompt);

    char *shell = g_find_program_in_path("sh");
    if (shell == NULL) {
        append(terminal, "sh not found\n");
        return;
    }

    const char *argv[] = { shell, "-c", command, NULL };
    char *output = unify_run(argv);
    append(terminal, output != NULL ? output : "(no output)");
    g_free(output);
    g_free(shell);
    gtk_editable_set_text(GTK_EDITABLE(terminal->entry), "");
}

static void on_open_emulator(GtkButton *button, gpointer data)
{
    (void)button;
    Terminal *terminal = data;
    const char *candidates[] = { "foot", "x-terminal-emulator", "gnome-terminal", NULL };

    for (int index = 0; candidates[index] != NULL; index++) {
        char *path = unify_tool(candidates[index]);
        if (path != NULL) {
            g_spawn_command_line_async(path, NULL);
            gchar *message = g_strdup_printf("Opened %s", candidates[index]);
            gtk_label_set_text(GTK_LABEL(terminal->status), message);
            g_free(message);
            g_free(path);
            return;
        }
    }
    gtk_label_set_text(GTK_LABEL(terminal->status),
                       "No terminal emulator installed. Install the 'foot' package.");
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    Terminal *terminal = g_new0(Terminal, 1);

    GtkWidget *window = unify_window(application, "Terminal", 640, 480);
    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    GtkWidget *bar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
    gtk_widget_set_margin_top(bar, 10);
    gtk_widget_set_margin_bottom(bar, 6);
    gtk_widget_set_margin_start(bar, 12);
    gtk_widget_set_margin_end(bar, 12);
    GtkWidget *open = gtk_button_new_with_label("Open terminal emulator");
    g_signal_connect(open, "clicked", G_CALLBACK(on_open_emulator), terminal);
    gtk_widget_set_hexpand(open, TRUE);
    gtk_widget_set_halign(open, GTK_ALIGN_START);
    gtk_box_append(GTK_BOX(bar), open);
    gtk_box_append(GTK_BOX(content), bar);

    terminal->status = gtk_label_new("Launches the system terminal emulator.");
    gtk_label_set_xalign(GTK_LABEL(terminal->status), 0.0f);
    gtk_widget_add_css_class(terminal->status, "dim-label");
    gtk_widget_set_margin_start(terminal->status, 12);
    gtk_widget_set_margin_bottom(terminal->status, 6);
    gtk_box_append(GTK_BOX(content), terminal->status);

    terminal->output = unify_log_view();
    gtk_widget_set_vexpand(terminal->output, TRUE);
    gtk_box_append(GTK_BOX(content), terminal->output);

    GtkWidget *entry_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
    gtk_widget_set_margin_top(entry_box, 6);
    gtk_widget_set_margin_bottom(entry_box, 10);
    gtk_widget_set_margin_start(entry_box, 12);
    gtk_widget_set_margin_end(entry_box, 12);
    terminal->entry = gtk_entry_new();
    gtk_entry_set_placeholder_text(GTK_ENTRY(terminal->entry),
                                   "Run a command (this runner is not a full terminal)");
    gtk_widget_set_hexpand(terminal->entry, TRUE);
    g_signal_connect(terminal->entry, "activate", G_CALLBACK(on_run), terminal);
    GtkWidget *run = gtk_button_new_with_label("Run");
    g_signal_connect(run, "clicked", G_CALLBACK(on_run), terminal);
    gtk_box_append(GTK_BOX(entry_box), terminal->entry);
    gtk_box_append(GTK_BOX(entry_box), run);
    gtk_box_append(GTK_BOX(content), entry_box);

    gtk_window_set_child(GTK_WINDOW(window), content);

    g_object_set_data_full(G_OBJECT(window), "terminal", terminal, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.Terminal", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
