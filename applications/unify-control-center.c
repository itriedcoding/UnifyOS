/* Unify Control Center — quick toggles for volume, brightness, radio state. */

#include "libunify.h"

#include <stdlib.h>

typedef struct {
    GtkWidget *volume;
    GtkWidget *brightness;
    GtkWidget *wifi;
    GtkWidget *bluetooth;
    GtkWidget *status;
    gboolean updating;
} ControlCenter;

static char *ctl(const char *command, const char *argument)
{
    char *program = unify_tool("unify-ctl");
    if (program == NULL)
        return NULL;

    const char *argv[5] = { program, command, NULL, "--json", NULL };
    if (argument != NULL) {
        argv[2] = argument;
        argv[4] = NULL;
    }
    char *output = unify_run(argv);
    g_free(program);
    return output;
}

static void run_action(ControlCenter *center, const char *command,
                       const char *argument, const char *message)
{
    char *program = unify_tool("unify-ctl");
    if (program == NULL) {
        gtk_label_set_text(GTK_LABEL(center->status), "unify-ctl is not installed");
        return;
    }

    const char *argv[6] = { program, command, NULL, NULL, NULL, NULL };
    if (argument != NULL) {
        argv[2] = argument;
    }
    if (command != NULL && argument != NULL && g_strcmp0(command, "volume") == 0) {
        argv[3] = g_strdup_printf("%d", (int)gtk_range_get_value(GTK_RANGE(center->volume)));
        gboolean ok = unify_run_ok(argv);
        g_free((char *)argv[3]);
        gtk_label_set_text(GTK_LABEL(center->status), ok ? message : "volume change failed");
        g_free(program);
        return;
    }
    if (command != NULL && argument != NULL && g_strcmp0(command, "brightness") == 0) {
        argv[3] = g_strdup_printf("%d", (int)gtk_range_get_value(GTK_RANGE(center->brightness)));
        gboolean ok = unify_run_ok(argv);
        g_free((char *)argv[3]);
        gtk_label_set_text(GTK_LABEL(center->status), ok ? message : "brightness change failed");
        g_free(program);
        return;
    }

    gboolean ok = unify_run_ok(argv);
    gtk_label_set_text(GTK_LABEL(center->status), ok ? message : "command failed");
    g_free(program);
}

static void refresh(ControlCenter *center)
{
    center->updating = TRUE;

    char *volume = ctl("volume", "status");
    char *brightness = ctl("brightness", "status");
    char *network = ctl("network", "status");
    char *bluetooth = ctl("bluetooth", "status");
    char *note = g_strdup("");

    if (volume != NULL) {
        gtk_range_set_value(GTK_RANGE(center->volume),
                            unify_json_number(volume, "percent", 0));
        g_free(note);
        note = g_strdup("");
    } else {
        g_free(note);
        note = g_strdup("Audio backend not available. ");
    }
    if (brightness != NULL) {
        gtk_range_set_value(GTK_RANGE(center->brightness),
                            unify_json_number(brightness, "percent", 0));
    } else {
        char *combined = g_strconcat(note, "No backlight device. ", NULL);
        g_free(note);
        note = combined;
    }
    if (network != NULL) {
        char *radio = unify_json_string(network, "wifi_radio");
        gtk_switch_set_active(GTK_SWITCH(center->wifi),
                              radio != NULL && g_strcmp0(radio, "enabled") == 0);
        g_free(radio);
        g_free(note);
        note = g_strdup("");
    } else {
        char *combined = g_strconcat(note, "NetworkManager not running. ", NULL);
        g_free(note);
        note = combined;
    }
    if (bluetooth != NULL) {
        gtk_switch_set_active(GTK_SWITCH(center->bluetooth),
                              unify_json_bool(bluetooth, "powered", FALSE));
        g_free(note);
        note = g_strdup("");
    } else {
        char *combined = g_strconcat(note, "Bluetooth adapter not found. ", NULL);
        g_free(note);
        note = combined;
    }

    if (*note == '\0') {
        gtk_label_set_text(GTK_LABEL(center->status), "Backends online.");
    } else {
        gtk_label_set_text(GTK_LABEL(center->status), note);
    }

    g_free(note);
    g_free(volume);
    g_free(brightness);
    g_free(network);
    g_free(bluetooth);
    center->updating = FALSE;
}

static void on_volume_apply(GtkButton *button, gpointer data)
{
    (void)button;
    run_action((ControlCenter *)data, "volume", "set", "Volume updated.");
}

static void on_brightness_apply(GtkButton *button, gpointer data)
{
    (void)button;
    run_action((ControlCenter *)data, "brightness", "set", "Brightness updated.");
}

static void on_wifi_toggled(GtkSwitch *sw, GParamSpec *spec, gpointer data)
{
    (void)spec;
    ControlCenter *center = data;
    if (center->updating)
        return;
    run_action(center, "network", gtk_switch_get_active(sw) ? "wifi-on" : "wifi-off",
               "Wi-Fi state changed.");
}

static void on_bluetooth_toggled(GtkSwitch *sw, GParamSpec *spec, gpointer data)
{
    (void)spec;
    ControlCenter *center = data;
    if (center->updating)
        return;
    run_action(center, "bluetooth", gtk_switch_get_active(sw) ? "on" : "off",
               "Bluetooth state changed.");
}

static GtkWidget *slider_row(const char *title, GtkWidget *scale)
{
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
    gtk_widget_set_margin_top(box, 8);
    gtk_widget_set_margin_bottom(box, 8);
    gtk_widget_set_margin_start(box, 12);
    gtk_widget_set_margin_end(box, 12);
    GtkWidget *label = gtk_label_new(title);
    gtk_label_set_xalign(GTK_LABEL(label), 0.0f);
    gtk_box_append(GTK_BOX(box), label);
    gtk_box_append(GTK_BOX(box), scale);
    return box;
}

static GtkWidget *switch_row(const char *title, GtkWidget *sw)
{
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 12);
    gtk_widget_set_margin_top(box, 10);
    gtk_widget_set_margin_bottom(box, 10);
    gtk_widget_set_margin_start(box, 12);
    gtk_widget_set_margin_end(box, 12);
    GtkWidget *label = gtk_label_new(title);
    gtk_label_set_xalign(GTK_LABEL(label), 0.0f);
    gtk_widget_set_hexpand(label, TRUE);
    gtk_box_append(GTK_BOX(box), label);
    gtk_box_append(GTK_BOX(box), sw);
    return box;
}

static void on_activate(GtkApplication *application, gpointer data)
{
    (void)data;
    ControlCenter *center = g_new0(ControlCenter, 1);

    GtkWidget *window = unify_window(application, "Control Center", 420, 560);
    GtkWidget *content = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);

    center->volume = gtk_scale_new_with_range(GTK_ORIENTATION_HORIZONTAL, 0, 100, 1);
    gtk_scale_set_draw_value(GTK_SCALE(center->volume), TRUE);
    center->brightness = gtk_scale_new_with_range(GTK_ORIENTATION_HORIZONTAL, 0, 100, 1);
    gtk_scale_set_draw_value(GTK_SCALE(center->brightness), TRUE);

    gtk_box_append(GTK_BOX(content), unify_heading("Sound"));
    gtk_box_append(GTK_BOX(content), slider_row("Output volume", center->volume));
    GtkWidget *volume_apply = gtk_button_new_with_label("Apply volume");
    gtk_widget_set_margin_start(volume_apply, 12);
    gtk_widget_set_margin_end(volume_apply, 12);
    g_signal_connect(volume_apply, "clicked", G_CALLBACK(on_volume_apply), center);
    gtk_box_append(GTK_BOX(content), volume_apply);

    gtk_box_append(GTK_BOX(content), unify_heading("Display"));
    gtk_box_append(GTK_BOX(content), slider_row("Backlight", center->brightness));
    GtkWidget *brightness_apply = gtk_button_new_with_label("Apply brightness");
    gtk_widget_set_margin_start(brightness_apply, 12);
    gtk_widget_set_margin_end(brightness_apply, 12);
    g_signal_connect(brightness_apply, "clicked", G_CALLBACK(on_brightness_apply), center);
    gtk_box_append(GTK_BOX(content), brightness_apply);

    gtk_box_append(GTK_BOX(content), unify_heading("Radios"));
    center->wifi = gtk_switch_new();
    g_signal_connect(center->wifi, "notify::active", G_CALLBACK(on_wifi_toggled), center);
    gtk_box_append(GTK_BOX(content), switch_row("Wi-Fi", center->wifi));
    center->bluetooth = gtk_switch_new();
    g_signal_connect(center->bluetooth, "notify::active",
                     G_CALLBACK(on_bluetooth_toggled), center);
    gtk_box_append(GTK_BOX(content), switch_row("Bluetooth", center->bluetooth));

    center->status = gtk_label_new("Reading state…");
    gtk_label_set_wrap(GTK_LABEL(center->status), TRUE);
    gtk_widget_add_css_class(center->status, "dim-label");
    gtk_widget_set_margin_top(center->status, 12);
    gtk_widget_set_margin_start(center->status, 12);
    gtk_widget_set_margin_end(center->status, 12);
    gtk_box_append(GTK_BOX(content), center->status);

    gtk_window_set_child(GTK_WINDOW(window), unify_scrolled(content));
    refresh(center);

    g_object_set_data_full(G_OBJECT(window), "center", center, g_free);
    gtk_window_present(GTK_WINDOW(window));
}

int main(int argc, char **argv)
{
    GtkApplication *application =
        gtk_application_new("org.unifyos.ControlCenter", G_APPLICATION_DEFAULT_FLAGS);
    g_signal_connect(application, "activate", G_CALLBACK(on_activate), NULL);
    int status = g_application_run(G_APPLICATION(application), argc, argv);
    g_object_unref(application);
    return status;
}
