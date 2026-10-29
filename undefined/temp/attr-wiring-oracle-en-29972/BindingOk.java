class BindingOk {
    int ok(Object o) {
        if (o instanceof String s && s.length() > 0) {
            return s.length();
        }
        return 0;
    }
}
