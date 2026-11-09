/* Unify Updater — list available upgrades and apply them through unify-pkg. */

#include "libunify.h"

#include <stdlib.h>

typedef struct {
    GtkWidget *updates;
    GtkWidget *log;
    GtkWidget *status;
} Updater;

static void clear_updates(Updater *updater)
{
    GtkListBoxRow *row;
    while ((row = gtk_list_box_get_row_at_index(GTK_LIST_BOX(updater->updates), 0)) != NULL)
        gtk_list_box_remove(GTK_LIST_BOX(updater->updates), GTK_WIDGET(row));
}

static char *pkg(const char *command, const char *argument, gboolean json)
{
    char *program = unify_tool("unify-pkg");
    if (program == NULL)
        return NULL;

    const char *argv[5] = { program, command, NULL, NULL, NULL };
    int index = 2;
    if (argument != NULL)
        argv[index++] = argument;
    if (json)
        argv[index++] = "--json";
    argv[index] = NULL;

    char *output = unify_run(argv);
    g_free(program);
    return output;
}

static void on_check(GtkButton *button, gpointer data)
{
    (void)button;
    Updater *updater = data;
    char *json = pkg("upgradable", NULL, TRUE);
    clear_updates(updater);

    if (json == NULL) {
        gtk_label_set_text(GTK_LABEL(updater->status),
                           "Cannot query updates: unify-pkg is not installed.");
        return;
    }

    int count = 0;
    const char *cursor = strchr(json, '{');
    while (cursor != NULL && count < 200) {
        const char *next = strstr(cursor, "},{");
        gsize length = next != NULL ? (gsize)(next - cursor + 1) : strlen(cursor);
        gchar *chunk = g_strndup(cursor, length);
        gchar *name = unify_json_string(chunk, "package");
        gchar *installed = unify_json_string(chunk, "installed_version");
        gchar *available = unify_json_string(chunk, "available_version");
        if (name != NULL) {
            gchar *subtitle = g_strdup_printf("%s → %s",
                                              installed != NULL ? installed : "?",
                                              available != NULL ? available : "?");
            gtk_list_box_append(GTK_LIST_BOX(updater->updates), unify_row(name, subtitle));
            g_free(subtitle);
            count++;
        }
        g_free(name);
        g_free(installed);
        g_free(available);
        g_free(chunk);
        cursor = next != NULL ? next + 3 : NULL;
    }

    gchar *message = count == 0
                         ? g_strdup("The system is up to date.")
                         : g_strdup_printf("%d update(s) available.", count);
    gtk_label_set_text(GTK_LABEL(updater->status), message);
    g_free(message);
    g_free(json);
}

static void on_upgrade(GtkButton *button, gpointer data)
{
    (void)button;
    Updater *updater = data;
    gtk_label_set_text(GTK_LABEL(updater->status), "Upgrading, this can take a while…");

    char *output = pkg("upgrade", NULL, FALSE);
    if (output == NULL) {
        unify_log_append(updater->log, "unify-pkg upgrade failed or is unavailable.\n");
        gtk_label_set_text(GTK_LABEL(updater->status), "Upgrade failed.");
        return;
    }
    unify_log_append(updater->log, output);
    unify_log_append(updater->log, "\n");
    gtk_label_set_text(GTK_LABEL(updater->status), "Upgrade finished, see the log.");
    g_free(output);
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    Updater *updater = g_new0(Updater, 1);

    GtkWidget *window = unify_window(application, "Updater", 560, 620);
    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    GtkWidget *bar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
    gtk_widget_set_margin_top(bar, 10);
    gtk_widget_set_margin_bottom(bar, 6);
    gtk_widget_set_margin_start(bar, 12);
    gtk_widget_set_margin_end(bar, 12);
    GtkWidget *check = gtk_button_new_with_label("Check for updates");
    GtkWidget *upgrade = gtk_button_new_with_label("Install updates");
    g_signal_connect(check, "clicked", G_CALLBACK(on_check), updater);
    g_signal_connect(upgrade, "clicked", G_CALLBACK(on_upgrade), updater);
    gtk_box_append(GTK_BOX(bar), check);
    gtk_box_append(GTK_BOX(bar), upgrade);
    gtk_box_append(GTK_BOX(content), bar);

    updater->status = gtk_label_new("Not checked yet.");
    gtk_widget_add_css_class(updater->status, "dim-label");
    gtk_label_set_xalign(GTK_LABEL(updater->status), 0.0f);
    gtk_widget_set_margin_start(updater->status, 12);
    gtk_widget_set_margin_bottom(updater->status, 6);
    gtk_box_append(GTK_BOX(content), updater->status);

    updater->updates = gtk_list_box_new();
    gtk_list_box_set_selection_mode(GTK_LIST_BOX(updater->updates), GTK_SELECTION_NONE);
    gtk_widget_add_css_class(updater->updates, "boxed-list");
    gtk_widget_set_margin_start(updater->updates, 12);
    gtk_widget_set_margin_end(updater->updates, 12);
    gtk_box_append(GTK_BOX(content), unify_scrolled(updater->updates));

    gtk_box_append(GTK_BOX(content), unify_heading("Log"));
    updater->log = unify_log_view();
    gtk_widget_set_size_request(updater->log, -1, 160);
    gtk_widget_set_margin_start(updater->log, 12);
    gtk_widget_set_margin_end(updater->log, 12);
    gtk_widget_set_margin_bottom(updater->log, 12);
    gtk_box_append(GTK_BOX(content), updater->log);

    gtk_window_set_child(GTK_WINDOW(window), content);

    g_object_set_data_full(G_OBJECT(window), "updater", updater, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.Updater", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
