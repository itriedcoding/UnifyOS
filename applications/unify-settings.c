/* Unify Settings — system identity, appearance and shortcuts to the panels. */

#include "libunify.h"

#include <stdlib.h>

typedef struct {
    GtkWidget *details;
    GtkWidget *dark_switch;
    GtkWidget *status;
} Settings;

static char *ctl(const char *command, const char *argument)
{
    char *program = unify_tool("unify-ctl");
    if (program == NULL)
        return NULL;
    const char *argv[5] = { program, command, NULL, "--json", NULL };
    if (argument != NULL)
        argv[2] = argument;
    char *output = unify_run(argv);
    g_free(program);
    return output;
}

static void clear_list(GtkWidget *list)
{
    GtkListBoxRow *row;
    while ((row = gtk_list_box_get_row_at_index(GTK_LIST_BOX(list), 0)) != NULL)
        gtk_list_box_remove(GTK_LIST_BOX(list), GTK_WIDGET(row));
}

static void refresh(Settings *settings)
{
    clear_list(settings->details);

    char *info = ctl("sysinfo", NULL);
    if (info == NULL) {
        gtk_list_box_append(GTK_LIST_BOX(settings->details),
                            unify_row("unify-ctl not found",
                                      "Install the UnifyOS system tools to see hardware details."));
        return;
    }

    char *hostname = unify_json_string(info, "hostname");
    char *kernel = unify_json_string(info, "kernel");
    char *version = unify_json_string(info, "unify_version");
    double uptime = unify_json_number(info, "uptime_seconds", 0);
    double cores = unify_json_number(info, "cores", 0);
    double memory = unify_json_number(info, "total_kb", 0);
    char *detail = NULL;

    detail = g_strdup_printf("%s · UnifyOS %s", hostname != NULL ? hostname : "unknown",
                             version != NULL ? version : "?");
    gtk_list_box_append(GTK_LIST_BOX(settings->details), unify_row("Computer", detail));
    g_free(detail);

    detail = g_strdup_printf("Linux %s · %d cores · %.0f MiB RAM",
                             kernel != NULL ? kernel : "?", (int)cores, memory / 1024.0);
    gtk_list_box_append(GTK_LIST_BOX(settings->details), unify_row("System", detail));
    g_free(detail);

    detail = g_strdup_printf("Up for %d hours %d minutes", (int)(uptime / 3600),
                             (int)((long)uptime % 3600 / 60));
    gtk_list_box_append(GTK_LIST_BOX(settings->details), unify_row("Uptime", detail));
    g_free(detail);

    char *disk = ctl("disk", NULL);
    if (disk != NULL) {
        double percent = unify_json_number(strstr(disk, "\"storage\"") != NULL
                                               ? strstr(disk, "\"storage\"")
                                               : disk,
                                           "used_percent", -1);
        if (percent >= 0) {
            detail = g_strdup_printf("%.0f %% of the root filesystem used", percent);
            gtk_list_box_append(GTK_LIST_BOX(settings->details), unify_row("Storage", detail));
            g_free(detail);
        }
        g_free(disk);
    }

    g_free(hostname);
    g_free(kernel);
    g_free(version);
    g_free(info);
}

static void on_dark_toggled(GtkSwitch *sw, GParamSpec *spec, gpointer data)
{
    (void)spec;
    (void)data;
    GtkSettings *gtk_settings = gtk_settings_get_default();
    g_object_set(gtk_settings, "gtk-application-prefer-dark-theme",
                 gtk_switch_get_active(sw), NULL);
}

static void launch(const char *program)
{
    char *path = unify_tool(program);
    if (path == NULL) {
        return;
    }
    g_spawn_command_line_async(path, NULL);
    g_free(path);
}

static void on_open_control_center(GtkButton *button, gpointer data)
{
    (void)button;
    (void)data;
    launch("unify-control-center");
}

static void on_open_monitor(GtkButton *button, gpointer data)
{
    (void)button;
    (void)data;
    launch("unify-monitor");
}

static void on_open_power(GtkButton *button, gpointer data)
{
    (void)button;
    Settings *settings = data;
    char *program = unify_tool("unify-ctl");
    if (program == NULL) {
        gtk_label_set_text(GTK_LABEL(settings->status), "unify-ctl is not installed");
        return;
    }
    const char *argv[] = { program, "power", "menu", NULL };
    gboolean ok = unify_run_ok(argv);
    gtk_label_set_text(GTK_LABEL(settings->status),
                       ok ? "Power menu opened." : "Power menu is not available here.");
    g_free(program);
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    Settings *settings = g_new0(Settings, 1);

    GtkWidget *window = unify_window(application, "Settings", 460, 620);
    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    gtk_box_append(GTK_BOX(content), unify_heading("System information"));
    settings->details = gtk_list_box_new();
    gtk_list_box_set_selection_mode(GTK_LIST_BOX(settings->details), GTK_SELECTION_NONE);
    gtk_widget_add_css_class(settings->details, "boxed-list");
    gtk_widget_set_margin_start(settings->details, 12);
    gtk_widget_set_margin_end(settings->details, 12);
    gtk_box_append(GTK_BOX(content), settings->details);

    gtk_box_append(GTK_BOX(content), unify_heading("Appearance"));
    GtkWidget *appearance = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 12);
    gtk_widget_set_margin_start(appearance, 12);
    gtk_widget_set_margin_end(appearance, 12);
    GtkWidget *dark_label = gtk_label_new("Dark theme");
    gtk_label_set_xalign(GTK_LABEL(dark_label), 0.0f);
    gtk_widget_set_hexpand(dark_label, TRUE);
    settings->dark_switch = gtk_switch_new();
    GtkSettings *gtk_settings = gtk_settings_get_default();
    gboolean dark = FALSE;
    g_object_get(gtk_settings, "gtk-application-prefer-dark-theme", &dark, NULL);
    gtk_switch_set_active(GTK_SWITCH(settings->dark_switch), dark);
    g_signal_connect(settings->dark_switch, "notify::active",
                     G_CALLBACK(on_dark_toggled), settings);
    gtk_box_append(GTK_BOX(appearance), dark_label);
    gtk_box_append(GTK_BOX(appearance), settings->dark_switch);
    gtk_box_append(GTK_BOX(content), appearance);

    gtk_box_append(GTK_BOX(content), unify_heading("Panels"));
    GtkWidget *panels = gtk_box_new(GTK_ORIENTATION_VERTICAL, 6);
    gtk_widget_set_margin_start(panels, 12);
    gtk_widget_set_margin_end(panels, 12);
    GtkWidget *control = gtk_button_new_with_label("Open Control Center");
    GtkWidget *monitor = gtk_button_new_with_label("Open System Monitor");
    GtkWidget *power = gtk_button_new_with_label("Power menu");
    g_signal_connect(control, "clicked", G_CALLBACK(on_open_control_center), settings);
    g_signal_connect(monitor, "clicked", G_CALLBACK(on_open_monitor), settings);
    g_signal_connect(power, "clicked", G_CALLBACK(on_open_power), settings);
    gtk_box_append(GTK_BOX(panels), control);
    gtk_box_append(GTK_BOX(panels), monitor);
    gtk_box_append(GTK_BOX(panels), power);
    gtk_box_append(GTK_BOX(content), panels);

    settings->status = gtk_label_new("");
    gtk_widget_add_css_class(settings->status, "dim-label");
    gtk_widget_set_margin_top(settings->status, 12);
    gtk_widget_set_margin_start(settings->status, 12);
    gtk_box_append(GTK_BOX(content), settings->status);

    gtk_window_set_child(GTK_WINDOW(window), unify_scrolled(content));
    refresh(settings);

    g_object_set_data_full(G_OBJECT(window), "settings", settings, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.Settings", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
