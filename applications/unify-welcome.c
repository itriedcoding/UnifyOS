/* Unify Welcome — first-run orientation with real entry points. */

#include "libunify.h"

#include <stdlib.h>

typedef struct {
    GtkWidget *status;
} Welcome;

static void launch(const char *program, GtkWidget *status)
{
    char *path = unify_tool(program);
    if (path == NULL) {
        gchar *message = g_strdup_printf("%s is not installed", program);
        gtk_label_set_text(GTK_LABEL(status), message);
        g_free(message);
        return;
    }
    g_spawn_command_line_async(path, NULL);
    g_free(path);
}

static void on_install(GtkButton *button, gpointer data)
{
    (void)button;
    launch("unify-installer", ((Welcome *)data)->status);
}

static void on_settings(GtkButton *button, gpointer data)
{
    (void)button;
    launch("unify-settings", ((Welcome *)data)->status);
}

static void on_store(GtkButton *button, gpointer data)
{
    (void)button;
    launch("unify-store", ((Welcome *)data)->status);
}

static void on_docs(GtkButton *button, gpointer data)
{
    (void)button;
    Welcome *welcome = data;
    const char *candidates[] = { "/usr/share/doc/unify", "/usr/share/doc/unifyos", NULL };
    for (int index = 0; candidates[index] != NULL; index++) {
        if (!g_file_test(candidates[index], G_FILE_TEST_EXISTS))
            continue;
        gchar *command = g_strdup_printf("xdg-open '%s'", candidates[index]);
        g_spawn_command_line_async(command, NULL);
        g_free(command);
        gtk_label_set_text(GTK_LABEL(welcome->status), "Opened the documentation.");
        return;
    }
    gtk_label_set_text(GTK_LABEL(welcome->status), "Documentation is not installed.");
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    Welcome *welcome = g_new0(Welcome, 1);

    GtkWidget *window = unify_window(application, "Welcome to UnifyOS", 520, 620);
    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_widget_set_margin_start(content, 18);
    gtk_widget_set_margin_end(content, 18);

    GtkWidget *title = gtk_label_new("Welcome to UnifyOS");
    gtk_widget_add_css_class(title, "title-1");
    gtk_label_set_xalign(GTK_LABEL(title), 0.0f);
    gtk_widget_set_margin_top(title, 18);
    gtk_box_append(GTK_BOX(content), title);

    char *release = NULL;
    if (g_file_get_contents("/etc/unify-release", &release, NULL, NULL)) {
        GtkWidget *body = gtk_label_new(release);
        gtk_label_set_wrap(GTK_LABEL(body), TRUE);
        gtk_label_set_xalign(GTK_LABEL(body), 0.0f);
        gtk_box_append(GTK_BOX(content), body);
        g_free(release);
    } else {
        GtkWidget *body = gtk_label_new("A lightweight Debian-based system with the Unify desktop.");
        gtk_label_set_wrap(GTK_LABEL(body), TRUE);
        gtk_label_set_xalign(GTK_LABEL(body), 0.0f);
        gtk_box_append(GTK_BOX(content), body);
    }

    gtk_box_append(GTK_BOX(content), unify_heading("Get started"));

    GtkWidget *actions = gtk_box_new(GTK_ORIENTATION_VERTICAL, 8);
    gtk_widget_set_margin_top(actions, 6);
    GtkWidget *install = gtk_button_new_with_label("Install UnifyOS to this computer");
    GtkWidget *settings = gtk_button_new_with_label("Open Settings");
    GtkWidget *store = gtk_button_new_with_label("Browse the Store");
    GtkWidget *docs = gtk_button_new_with_label("Read the documentation");
    g_signal_connect(install, "clicked", G_CALLBACK(on_install), welcome);
    g_signal_connect(settings, "clicked", G_CALLBACK(on_settings), welcome);
    g_signal_connect(store, "clicked", G_CALLBACK(on_store), welcome);
    g_signal_connect(docs, "clicked", G_CALLBACK(on_docs), welcome);
    gtk_box_append(GTK_BOX(actions), install);
    gtk_box_append(GTK_BOX(actions), settings);
    gtk_box_append(GTK_BOX(actions), store);
    gtk_box_append(GTK_BOX(actions), docs);
    gtk_box_append(GTK_BOX(content), actions);

    gtk_box_append(GTK_BOX(content), unify_heading("About"));
    GtkWidget *about = gtk_label_new(
        "UnifyOS 0.1.0 — Debian 12 base, labwc desktop, Rust system services.\n"
        "The installer writes to a disk and cannot be undone.");
    gtk_label_set_wrap(GTK_LABEL(about), TRUE);
    gtk_label_set_xalign(GTK_LABEL(about), 0.0f);
    gtk_widget_add_css_class(about, "dim-label");
    gtk_box_append(GTK_BOX(content), about);

    welcome->status = gtk_label_new("");
    gtk_widget_add_css_class(welcome->status, "dim-label");
    gtk_label_set_wrap(GTK_LABEL(welcome->status), TRUE);
    gtk_label_set_xalign(GTK_LABEL(welcome->status), 0.0f);
    gtk_widget_set_margin_top(welcome->status, 12);
    gtk_box_append(GTK_BOX(content), welcome->status);

    gtk_window_set_child(GTK_WINDOW(window), unify_scrolled(content));

    g_object_set_data_full(G_OBJECT(window), "welcome", welcome, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.Welcome", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
