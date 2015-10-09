from sphinxcontrib.domaintools import custom_domain

def setup(app):
    app.add_domain(custom_domain('LithosOptions',
        name  = 'lithos',
        label = "Lithos Yaml Options",

        elements = dict(
            opt = dict(
                objname      = "Yaml Option",
                indextemplate = "pair: %s; Option",
            ),
            volume = dict(
                objname      = "Volume Type",
                indextemplate = "pair: %s; Volume Type",
            ),
        )))
