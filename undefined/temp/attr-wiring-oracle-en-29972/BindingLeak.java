class BindingLeak {
    int leak(Object o) {
        if ((o instanceof String s) == true) {
            return s.length();
        }
        return 0;
    }
}
