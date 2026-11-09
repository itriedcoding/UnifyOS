/* Unify Store — search the Debian archive and install packages via unify-pkg. */

#include "libunify.h"

#include <stdlib.h>

typedef struct {
    GtkWidget *search;
    GtkWidget *results;
    GtkWidget *log;
    GtkWidget *status;
    char *selected;
} Store;

static void log_line(Store *store, const char *text)
{
    gchar *stamped = g_strdup_printf("%s\n", text);
    unify_log_append(store->log, stamped);
    g_free(stamped);
}

static void clear_results(Store *store)
{
    GtkListBoxRow *row;
    while ((row = gtk_list_box_get_row_at_index(GTK_LIST_BOX(store->results), 0)) != NULL)
        gtk_list_box_remove(GTK_LIST_BOX(store->results), GTK_WIDGET(row));
}

static void on_row_activated(GtkListBox *list, GtkListBoxRow *row, gpointer data)
{
    (void)list;
    Store *store = data;
    const char *name = g_object_get_data(G_OBJECT(row), "package");
    g_free(store->selected);
    store->selected = g_strdup(name);
    gchar *message = g_strdup_printf("Selected %s", name != NULL ? name : "nothing");
    gtk_label_set_text(GTK_LABEL(store->status), message);
    g_free(message);
}

static void on_search(GtkButton *button, gpointer data)
{
    (void)button;
    Store *store = data;
    const char *term = gtk_editable_get_text(GTK_EDITABLE(store->search));
    if (term == NULL || *term == '\0') {
        gtk_label_set_text(GTK_LABEL(store->status), "Type a package name to search.");
        return;
    }

    char *program = unify_tool("unify-pkg");
    if (program == NULL) {
        gtk_label_set_text(GTK_LABEL(store->status), "unify-pkg is not installed.");
        return;
    }

    const char *argv[] = { program, "search", term, "--json", NULL };
    char *json = unify_run(argv);
    clear_results(store);

    if (json == NULL) {
        gtk_label_set_text(GTK_LABEL(store->status), "Search failed: unify-pkg is unavailable.");
        g_free(program);
        return;
    }

    int count = 0;
    const char *cursor = strchr(json, '{');
    while (cursor != NULL && count < 40) {
        const char *next = strstr(cursor, "},{");
        gsize length = next != NULL ? (gsize)(next - cursor + 1) : strlen(cursor);
        gchar *chunk = g_strndup(cursor, length);
        gchar *name = unify_json_string(chunk, "package");
        gchar *description = unify_json_string(chunk, "description");
        if (name != NULL) {
            GtkWidget *row = unify_row(name, description);
            g_object_set_data_full(G_OBJECT(row), "package", g_strdup(name), g_free);
            gtk_list_box_append(GTK_LIST_BOX(store->results), row);
            count++;
        }
        g_free(name);
        g_free(description);
        g_free(chunk);
        cursor = next != NULL ? next + 3 : NULL;
    }

    gchar *message = g_strdup_printf("%d result(s) for “%s”", count, term);
    gtk_label_set_text(GTK_LABEL(store->status), message);
    g_free(message);
    g_free(json);
    g_free(program);
}

static void on_install(GtkButton *button, gpointer data)
{
    (void)button;
    Store *store = data;
    if (store->selected == NULL) {
        gtk_label_set_text(GTK_LABEL(store->status), "Select a package first.");
        return;
    }

    char *program = unify_tool("unify-pkg");
    if (program == NULL) {
        gtk_label_set_text(GTK_LABEL(store->status), "unify-pkg is not installed.");
        return;
    }

    gchar *message = g_strdup_printf("Installing %s…", store->selected);
    gtk_label_set_text(GTK_LABEL(store->status), message);
    g_free(message);

    const char *argv[] = { program, "install", store->selected, NULL };
    char *output = unify_run(argv);
    log_line(store, output != NULL ? output : "install failed: no output");
    gtk_label_set_text(GTK_LABEL(store->status),
                       output != NULL ? "Install finished, see the log." : "Install failed.");
    g_free(output);
    g_free(program);
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    Store *store = g_new0(Store, 1);

    GtkWidget *window = unify_window(application, "Store", 560, 640);
    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    GtkWidget *search_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
    gtk_widget_set_margin_top(search_box, 10);
    gtk_widget_set_margin_bottom(search_box, 6);
    gtk_widget_set_margin_start(search_box, 12);
    gtk_widget_set_margin_end(search_box, 12);
    store->search = gtk_entry_new();
    gtk_entry_set_placeholder_text(GTK_ENTRY(store->search), "Package name");
    gtk_widget_set_hexpand(store->search, TRUE);
    GtkWidget *search_button = gtk_button_new_with_label("Search");
    g_signal_connect(store->search, "activate", G_CALLBACK(on_search), store);
    g_signal_connect(search_button, "clicked", G_CALLBACK(on_search), store);
    gtk_box_append(GTK_BOX(search_box), store->search);
    gtk_box_append(GTK_BOX(search_box), search_button);
    gtk_box_append(GTK_BOX(content), search_box);

    store->status = gtk_label_new("Search the archive to install software.");
    gtk_label_set_xalign(GTK_LABEL(store->status), 0.0f);
    gtk_widget_add_css_class(store->status, "dim-label");
    gtk_widget_set_margin_start(store->status, 12);
    gtk_widget_set_margin_bottom(store->status, 6);
    gtk_box_append(GTK_BOX(content), store->status);

    store->results = gtk_list_box_new();
    gtk_list_box_set_selection_mode(GTK_LIST_BOX(store->results), GTK_SELECTION_SINGLE);
    gtk_widget_add_css_class(store->results, "boxed-list");
    gtk_widget_set_margin_start(store->results, 12);
    gtk_widget_set_margin_end(store->results, 12);
    g_signal_connect(store->results, "row-activated", G_CALLBACK(on_row_activated), store);
    GtkWidget *scroller = unify_scrolled(store->results);
    gtk_box_append(GTK_BOX(content), scroller);

    GtkWidget *install = gtk_button_new_with_label("Install selected package");
    gtk_widget_set_margin_top(install, 8);
    gtk_widget_set_margin_bottom(install, 8);
    gtk_widget_set_margin_start(install, 12);
    gtk_widget_set_margin_end(install, 12);
    g_signal_connect(install, "clicked", G_CALLBACK(on_install), store);
    gtk_box_append(GTK_BOX(content), install);

    gtk_box_append(GTK_BOX(content), unify_heading("Log"));
    store->log = unify_log_view();
    gtk_widget_set_size_request(store->log, -1, 140);
    gtk_box_append(GTK_BOX(content), store->log);

    gtk_window_set_child(GTK_WINDOW(window), content);

    g_object_set_data_full(G_OBJECT(window), "store", store, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.Store", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
